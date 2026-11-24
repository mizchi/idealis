#[path = "safety_functions.rs"]
mod functions;
use crate::ir::{Definition, Expression, Scalar};
use crate::safety_expressions::MembershipSet;
use crate::safety_fold_order::{OrderIndependence, classify};
use crate::safety_prepared::PreparedProgram;
#[cfg(not(target_arch = "wasm32"))]
use crate::safety_work::WorkBudget;
use crate::{CoreError, Value};
use functions::{Function, FunctionDomain, FunctionUpdate, FunctionValues};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hash, Hasher};
use std::sync::Arc;

#[derive(Debug, PartialEq, Eq)]
enum Cell {
    String(String),
    Set(Vec<Scalar>),
    Function(Function),
    ModelValue(String),
}

impl Hash for Cell {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::String(value) | Self::ModelValue(value) => value.hash(state),
            Self::Set(values) => values.hash(state),
            Self::Function(function) => function.hash(state),
        }
    }
}

struct ValueHasher(u64);
impl Default for ValueHasher {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}
impl Hasher for ValueHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.write_u64(u64::from(*byte));
        }
    }
    fn write_u64(&mut self, value: u64) {
        self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3) ^ value;
    }
    fn write_u32(&mut self, value: u32) {
        self.write_u64(u64::from(value));
    }
    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }
    fn write_i32(&mut self, value: i32) {
        self.write_u32(value as u32);
    }
    fn write_isize(&mut self, value: isize) {
        self.write_u64(value as u64);
    }
}

type RegisteredMembership = HashMap<usize, Option<Option<Arc<MembershipSet>>>>;

type MembershipCache = HashMap<(u64, Scalar), Vec<(Arc<MembershipSet>, bool)>>;

pub(crate) struct Arena<'a> {
    pub definitions: &'a [Definition],
    pub metadata: Option<Arc<crate::safety_definitions::Metadata>>,
    base: Option<&'a Arena<'a>>,
    prepared: Option<&'a PreparedProgram>,
    constants: Vec<Option<Scalar>>,
    offset: usize,
    #[cfg(not(target_arch = "wasm32"))]
    budget: Option<(&'a WorkBudget, usize)>,
    cells: Vec<Cell>,
    domains: HashMap<u64, Vec<Arc<FunctionDomain>>>,
    #[cfg(not(target_arch = "wasm32"))]
    aliases: Vec<Arc<FunctionDomain>>,
    depths: Vec<u8>,
    index: HashMap<u64, u32, BuildHasherDefault<ValueHasher>>,
    links: Vec<Option<u32>>,
    data: HashMap<Value, Scalar>,
    types: HashMap<Expression, Option<Arc<MembershipSet>>>,
    registered: RegisteredMembership,
    membership: MembershipCache,
    pub work: u32,
    maximum: u32,
}

impl<'scope> Arena<'scope> {
    pub fn new(maximum: u32) -> Self {
        Self {
            definitions: &[],
            metadata: None,
            base: None,
            prepared: None,
            constants: Vec::new(),
            offset: 0,
            #[cfg(not(target_arch = "wasm32"))]
            budget: None,
            cells: vec![],
            domains: HashMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            aliases: Vec::new(),
            depths: vec![],
            index: HashMap::default(),
            links: vec![],
            data: HashMap::new(),
            types: HashMap::new(),
            registered: HashMap::new(),
            membership: HashMap::new(),
            work: 0,
            maximum,
        }
    }
    pub fn with_prepared(maximum: u32, prepared: &'scope PreparedProgram) -> Self {
        Self {
            prepared: Some(prepared),
            constants: vec![None; prepared.len()],
            ..Self::new(maximum)
        }
    }
    pub fn data_expression(
        &mut self,
        expression: &Expression,
        value: &Value,
    ) -> Result<Scalar, CoreError> {
        match self.prepared.and_then(|prepared| prepared.slot(expression)) {
            Some(slot) => self.constant(slot, value),
            None => self.data(value),
        }
    }
    fn cached_constant(&self, slot: usize) -> Option<Scalar> {
        self.constants[slot].or_else(|| self.base.and_then(|base| base.cached_constant(slot)))
    }
    fn constant(&mut self, slot: usize, value: &Value) -> Result<Scalar, CoreError> {
        if let Some(value) = self.cached_constant(slot) {
            return Ok(value);
        }
        let value = self.data(value)?;
        self.constants[slot] = Some(value);
        Ok(value)
    }
    pub fn tick(&mut self) -> Result<(), CoreError> {
        self.consume(1)
    }
    pub(crate) fn bind_run(
        &self,
        branch: &crate::ir::Branch,
        position: usize,
    ) -> Option<&'scope crate::safety_expressions::BindRun> {
        self.prepared
            .and_then(|prepared| prepared.bind_run(branch, position))
    }
    pub fn consume(&mut self, count: usize) -> Result<(), CoreError> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some((budget, worker)) = self.budget {
            let used = budget.consume(worker, count)?;
            self.work += used;
            return if used as usize == count {
                Ok(())
            } else {
                Err(work_exhausted())
            };
        }
        if count > (self.maximum - self.work) as usize {
            self.work = self.maximum;
            return Err(work_exhausted());
        }
        self.work += count as u32;
        Ok(())
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn fork<'a>(&'a self, budget: &'a WorkBudget, worker: usize) -> Arena<'a> {
        let mut worker = Arena {
            definitions: self.definitions,
            metadata: self.metadata.clone(),
            base: Some(self),
            prepared: self.prepared,
            constants: vec![None; self.constants.len()],
            offset: self.offset + self.cells.len(),
            budget: Some((budget, worker)),
            ..Arena::new(self.maximum)
        };
        let domains = self.domains.values().flatten();
        if domains.clone().take(17).count() <= 16 {
            for domain in domains.filter(|domain| domain.root_owned && domain.keys.len() <= 64) {
                let root = domain.root.as_ref().unwrap_or(domain);
                let alias = Arc::new(FunctionDomain {
                    keys: domain.keys.clone(),
                    hash: domain.hash,
                    depth: domain.depth,
                    root_owned: true,
                    root: Some(Arc::clone(root)),
                });
                worker
                    .domains
                    .entry(alias.hash)
                    .or_default()
                    .push(Arc::clone(&alias));
                worker.aliases.push(alias);
            }
        }
        worker
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn into_delta(self) -> ValueDelta {
        ValueDelta {
            cells: self.cells,
            constants: self.constants,
            data: self.data,
            types: self.types,
            registered: self.registered,
            membership: self.membership,
            offset: self.offset,
            work: self.work,
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn merge(&mut self, delta: ValueDelta) -> Result<ValueRemap, CoreError> {
        let mut mapping = ValueRemap {
            offset: delta.offset,
            values: Vec::with_capacity(delta.cells.len()),
        };
        for cell in delta.cells {
            let cell = match cell {
                Cell::String(value) => Cell::String(value),
                Cell::ModelValue(value) => Cell::ModelValue(value),
                Cell::Set(values) => {
                    Cell::Set(values.into_iter().map(|value| mapping.get(value)).collect())
                }
                Cell::Function(values) => {
                    let domain = if values.domain.root_owned {
                        Arc::clone(values.domain.root.as_ref().unwrap_or(&values.domain))
                    } else {
                        self.function_domain(
                            values
                                .domain
                                .keys
                                .iter()
                                .map(|key| mapping.get(*key))
                                .collect(),
                        )
                    };
                    Cell::Function(values.remap(domain, &mapping))
                }
            };
            mapping.values.push(self.insert(cell, false)?);
        }
        for (value, scalar) in delta.data {
            self.data
                .entry(value)
                .or_insert_with(|| mapping.get(scalar));
        }
        for (slot, value) in delta.constants.into_iter().enumerate() {
            if self.constants[slot].is_none() {
                self.constants[slot] = value.map(|value| mapping.get(value));
            }
        }
        for (address, prepared) in delta.registered {
            let target = self.registered.entry(address).or_insert(None);
            if target.is_none() {
                *target =
                    prepared.map(|prepared| prepared.map(|prepared| prepared.remap(&mapping)));
            }
        }
        for (expression, prepared) in delta.types {
            self.types
                .entry(expression)
                .or_insert_with(|| prepared.map(|prepared| prepared.remap(&mapping)));
        }
        for ((_, value), results) in delta.membership {
            for (prepared, result) in results {
                self.cache_membership_result(prepared.remap(&mapping), mapping.get(value), result);
            }
        }
        Ok(mapping)
    }
    pub fn register_memberships(&mut self, expression: &'scope Expression) {
        crate::safety_expressions::register_memberships(expression, &mut |expression| {
            self.registered
                .entry(std::ptr::from_ref(expression) as usize)
                .or_insert(None);
        });
    }
    fn has_registered(&self, address: usize) -> bool {
        self.registered.contains_key(&address)
            || self.base.is_some_and(|base| base.has_registered(address))
    }
    pub fn prepared_membership(
        &self,
        expression: &Expression,
    ) -> Option<Option<Arc<MembershipSet>>> {
        if let Some(prepared) = self
            .registered
            .get(&(std::ptr::from_ref(expression) as usize))
        {
            return prepared.clone();
        }
        self.base
            .and_then(|base| base.prepared_membership(expression))
            .or_else(|| self.types.get(expression).cloned())
    }
    pub fn cache_membership_type(
        &mut self,
        expression: &Expression,
        prepared: Option<Arc<MembershipSet>>,
    ) {
        let address = std::ptr::from_ref(expression) as usize;
        if self.has_registered(address) {
            self.registered.insert(address, Some(prepared));
        } else {
            self.types.insert(expression.clone(), prepared);
        }
    }
    pub fn membership_result(&self, prepared: &Arc<MembershipSet>, value: Scalar) -> Option<bool> {
        self.membership
            .get(&(prepared.fingerprint(), value))
            .and_then(|entries| {
                entries
                    .iter()
                    .find(|(stored, _)| stored == prepared)
                    .map(|(_, result)| *result)
            })
            .or_else(|| {
                self.base
                    .and_then(|base| base.membership_result(prepared, value))
            })
    }
    pub fn cache_membership_result(
        &mut self,
        prepared: Arc<MembershipSet>,
        value: Scalar,
        result: bool,
    ) {
        let entries = self
            .membership
            .entry((prepared.fingerprint(), value))
            .or_default();
        if !entries.iter().any(|(stored, _)| *stored == prepared) {
            entries.push((prepared, result));
        }
    }
    pub fn has_domain(&self, function: Scalar, domain: Scalar) -> Result<bool, CoreError> {
        let Cell::Function(entries) = self.cell(function)? else {
            return Err(CoreError::protocol("Expected a function"));
        };
        let Ok(Cell::Set(keys)) = self.cell(domain) else {
            return Err(CoreError::protocol("Incomparable TLA values"));
        };
        if entries.len() != keys.len() {
            return Ok(false);
        }
        if &*entries.domain.keys == keys.as_slice() {
            return Ok(true);
        }
        for (actual, expected) in entries.domain.keys.iter().zip(keys) {
            if !self.equal(*actual, *expected)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub fn text(&self, value: Scalar) -> Option<&str> {
        if let Scalar::Ref(index) = value
            && let Ok(Cell::String(text)) = self.cell(Scalar::Ref(index))
        {
            return Some(text);
        }
        None
    }
    pub fn string_utf16(&mut self, units: &[u16]) -> Result<Scalar, CoreError> {
        self.consume(units.len())?;
        let text = String::from_utf16(units).map_err(|_| {
            CoreError::protocol("UTF-16 slice cannot be represented as Unicode scalar text")
        })?;
        self.string(text)
    }
    pub fn sequence_values(&mut self, value: Scalar) -> Result<Vec<Scalar>, CoreError> {
        if let Some(text) = self.text(value) {
            let units = text.encode_utf16().collect::<Vec<_>>();
            return units
                .into_iter()
                .map(|unit| self.string_utf16(&[unit]))
                .collect();
        }
        self.sequence(value)
    }
    fn cell(&self, value: Scalar) -> Result<&Cell, CoreError> {
        match value {
            Scalar::Ref(index) if (index as usize) < self.offset => self
                .base
                .ok_or_else(|| CoreError::protocol("Missing base value store"))?
                .cell(value),
            Scalar::Ref(index) => self
                .cells
                .get(index as usize - self.offset)
                .ok_or_else(|| CoreError::protocol("Invalid interned value")),
            _ => Err(CoreError::protocol("Expected a collection or string")),
        }
    }
    fn intern(&mut self, cell: Cell) -> Result<Scalar, CoreError> {
        self.insert(cell, true)
    }
    fn find(&self, hash: u64, cell: &Cell) -> Option<u32> {
        let mut candidate = self.index.get(&hash).copied();
        while let Some(index) = candidate {
            let local = index as usize - self.offset;
            if self.cells[local] == *cell {
                return Some(index);
            }
            candidate = self.links[local];
        }
        self.base.and_then(|base| base.find(hash, cell))
    }
    fn find_updated(&self, hash: u64, update: &FunctionUpdate<'_>) -> Option<u32> {
        let mut candidate = self.index.get(&hash).copied();
        while let Some(index) = candidate {
            let local = index as usize - self.offset;
            if let Cell::Function(function) = &self.cells[local]
                && update.matches(function)
            {
                return Some(index);
            }
            candidate = self.links[local];
        }
        self.base.and_then(|base| base.find_updated(hash, update))
    }
    fn depth(&self, index: u32) -> u8 {
        if (index as usize) < self.offset {
            self.base.unwrap().depth(index)
        } else {
            self.depths[index as usize - self.offset]
        }
    }
    fn insert(&mut self, cell: Cell, charge: bool) -> Result<Scalar, CoreError> {
        let mut hasher = ValueHasher::default();
        cell.hash(&mut hasher);
        let hash = hasher.finish();
        if let Some(index) = self.find(hash, &cell) {
            return Ok(Scalar::Ref(index));
        }
        self.insert_new(cell, hash, charge)
    }
    fn insert_new(&mut self, cell: Cell, hash: u64, charge: bool) -> Result<Scalar, CoreError> {
        let depth = |value: &Scalar| match value {
            Scalar::Ref(index) => self.depth(*index),
            _ => 0,
        };
        let depth = match &cell {
            Cell::String(_) | Cell::ModelValue(_) => 0,
            Cell::Set(values) => 1 + values.iter().map(depth).max().unwrap_or(0),
            Cell::Function(values) => values.depth(|value| depth(&value)),
        };
        if depth > 64 {
            return Err(CoreError::protocol("TLA value nesting exceeds 64"));
        }
        if charge {
            self.tick()?;
        }
        let index = u32::try_from(self.offset + self.cells.len())
            .map_err(|_| CoreError::protocol("Value store exhausted"))?;
        self.links.push(self.index.insert(hash, index));
        self.cells.push(cell);
        self.depths.push(depth);
        Ok(Scalar::Ref(index))
    }
    pub fn string(&mut self, value: String) -> Result<Scalar, CoreError> {
        self.intern(Cell::String(value))
    }
    pub fn data(&mut self, value: &Value) -> Result<Scalar, CoreError> {
        if let Some(result) = self
            .data
            .get(value)
            .or_else(|| self.base.and_then(|base| base.data.get(value)))
        {
            return Ok(*result);
        }
        let result = match value {
            Value::Bool(_) | Value::Int(_) => Scalar::parse(value)?,
            Value::String(value) => self.string(value.clone())?,
            Value::ModelValue(value) => {
                Value::ModelValue(value.clone()).normalize()?;
                self.intern(Cell::ModelValue(value.clone()))?
            }
            Value::List(values) => {
                let values = values
                    .iter()
                    .map(|value| self.data(value))
                    .collect::<Result<Vec<_>, _>>()?;
                self.tuple(values)?
            }
            Value::Set(values) => {
                let values = values
                    .iter()
                    .map(|value| self.data(value))
                    .collect::<Result<Vec<_>, _>>()?;
                self.set(values)?
            }
            Value::Record(values) => {
                let mut entries = vec![];
                for (name, value) in values {
                    let name = self.string(name.clone())?;
                    let value = self.data(value)?;
                    entries.push((name, value));
                }
                self.function(entries)?
            }
            Value::Map(values) => {
                let mut entries = vec![];
                for (key, value) in values {
                    let key = self.data(key)?;
                    let value = self.data(value)?;
                    entries.push((key, value));
                }
                self.function(entries)?
            }
            _ => return Err(CoreError::protocol("Unsupported TLA data value")),
        };
        self.data.insert(value.clone(), result);
        Ok(result)
    }
    fn rank(&self, value: Scalar) -> u8 {
        match value {
            Scalar::Bool(_) => 1,
            Scalar::Int(_) => 2,
            Scalar::Ref(_) => match self.cell(value) {
                Ok(Cell::ModelValue(_)) => 0,
                Ok(Cell::String(_)) => 3,
                Ok(Cell::Set(_)) => 4,
                _ => 5,
            },
        }
    }
    pub fn total(&self, left: Scalar, right: Scalar) -> Ordering {
        let rank = self.rank(left).cmp(&self.rank(right));
        if rank != Ordering::Equal {
            return rank;
        }
        match (left, right) {
            (Scalar::Bool(left), Scalar::Bool(right)) => left.cmp(&right),
            (Scalar::Int(left), Scalar::Int(right)) => left.cmp(&right),
            _ => match (self.cell(left), self.cell(right)) {
                (Ok(Cell::String(left)), Ok(Cell::String(right))) => {
                    left.encode_utf16().cmp(right.encode_utf16())
                }
                (Ok(Cell::ModelValue(left)), Ok(Cell::ModelValue(right))) => left.cmp(right),
                (Ok(Cell::Set(left)), Ok(Cell::Set(right))) => left
                    .len()
                    .cmp(&right.len())
                    .then_with(|| self.compare_lists(left, right)),
                (Ok(Cell::Function(left)), Ok(Cell::Function(right))) => {
                    let size = left.len().cmp(&right.len());
                    if size != Ordering::Equal {
                        return size;
                    }
                    for ((left_key, left_value), (right_key, right_value)) in
                        left.iter().zip(right.iter())
                    {
                        let comparison = self
                            .total(left_key, right_key)
                            .then_with(|| self.total(left_value, right_value));
                        if comparison != Ordering::Equal {
                            return comparison;
                        }
                    }
                    Ordering::Equal
                }
                _ => Ordering::Equal,
            },
        }
    }
    fn compare_lists(&self, left: &[Scalar], right: &[Scalar]) -> Ordering {
        for (left, right) in left.iter().zip(right.iter()) {
            let result = self.total(*left, *right);
            if result != Ordering::Equal {
                return result;
            }
        }
        left.len().cmp(&right.len())
    }
    pub fn model_membership(&self, value: Scalar) -> Result<bool, CoreError> {
        if matches!(value, Scalar::Ref(_))
            && let Cell::ModelValue(name) = self.cell(value)?
        {
            if name.len() > 2 && name.as_bytes()[1] == b'_' {
                return Err(CoreError::protocol(
                    "Cannot test a typed model value in a non-model type",
                ));
            }
            return Ok(true);
        }
        Ok(false)
    }
    fn model_comparison(&self, left: Scalar, right: Scalar) -> Result<bool, CoreError> {
        let name = |value| {
            if matches!(value, Scalar::Ref(_)) {
                match self.cell(value) {
                    Ok(Cell::ModelValue(name)) => Some(name.as_str()),
                    _ => None,
                }
            } else {
                None
            }
        };
        let left = name(left);
        let right = name(right);
        if left.is_none() && right.is_none() {
            return Ok(false);
        }
        let kind = |name: Option<&str>| {
            name.and_then(|name| {
                (name.len() > 2 && name.as_bytes()[1] == b'_').then(|| name.as_bytes()[0])
            })
        };
        if (kind(left).is_some()
            && (right.is_none() || (kind(right).is_some() && kind(left) != kind(right))))
            || (kind(right).is_some() && left.is_none())
        {
            return Err(CoreError::protocol("Incomparable typed model values"));
        }
        Ok(true)
    }
    pub fn comparable(&self, left: Scalar, right: Scalar) -> Result<(), CoreError> {
        if matches!(
            (left, right),
            (Scalar::Int(_), Scalar::Int(_)) | (Scalar::Bool(_), Scalar::Bool(_))
        ) {
            return Ok(());
        }
        if (matches!(left, Scalar::Ref(_)) && matches!(self.cell(left), Ok(Cell::ModelValue(_))))
            || (matches!(right, Scalar::Ref(_))
                && matches!(self.cell(right), Ok(Cell::ModelValue(_))))
        {
            return Ok(());
        }
        if self.rank(left) != self.rank(right) {
            return Err(CoreError::protocol("Incomparable TLA values"));
        }
        if matches!(left, Scalar::Int(_) | Scalar::Bool(_)) {
            return Ok(());
        }
        let pairs: Vec<(Scalar, Scalar)> = match (self.cell(left), self.cell(right)) {
            (Ok(Cell::Set(left)), Ok(Cell::Set(right))) if left.len() == right.len() => {
                left.iter().copied().zip(right.iter().copied()).collect()
            }
            (Ok(Cell::Function(left)), Ok(Cell::Function(right))) if left.len() == right.len() => {
                left.iter()
                    .zip(right.iter())
                    .flat_map(|(left, right)| [(left.0, right.0), (left.1, right.1)])
                    .collect()
            }
            _ => vec![],
        };
        for (left, right) in pairs {
            self.comparable(left, right)?;
            if self.total(left, right) != Ordering::Equal {
                break;
            }
        }
        Ok(())
    }
    pub fn equal(&self, left: Scalar, right: Scalar) -> Result<bool, CoreError> {
        if left == right {
            return Ok(true);
        }
        if matches!(
            (left, right),
            (Scalar::Int(_), Scalar::Int(_)) | (Scalar::Bool(_), Scalar::Bool(_))
        ) {
            return Ok(false);
        }
        if self.model_comparison(left, right)? {
            return Ok(false);
        }
        if self.rank(left) != self.rank(right) {
            return Err(CoreError::protocol("Incomparable TLA values"));
        }
        if matches!(left, Scalar::Int(_) | Scalar::Bool(_)) {
            return Ok(false);
        }
        match (self.cell(left), self.cell(right)) {
            (Ok(Cell::Function(left)), Ok(Cell::Function(right))) => {
                if left.len() != right.len() {
                    return Ok(false);
                }
                for (left, right) in left.iter().zip(right.iter()) {
                    if !self.equal(left.0, right.0)? {
                        return Ok(false);
                    }
                    if !self.equal(left.1, right.1)? {
                        return Ok(false);
                    }
                }
            }
            (Ok(Cell::Set(left)), Ok(Cell::Set(right))) => {
                if left.len() != right.len() {
                    return Ok(false);
                }
                for (left, right) in left.iter().zip(right.iter()) {
                    if !self.equal(*left, *right)? {
                        return Ok(false);
                    }
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
    pub fn set(&mut self, mut values: Vec<Scalar>) -> Result<Scalar, CoreError> {
        values = self.ordered(values)?;
        self.intern(Cell::Set(values))
    }
    pub fn ordered(&self, mut values: Vec<Scalar>) -> Result<Vec<Scalar>, CoreError> {
        values.sort_by(|left, right| self.total(*left, *right));
        for pair in values.windows(2) {
            self.comparable(pair[0], pair[1])?;
        }
        values.dedup();
        Ok(values)
    }
    pub fn tuple(&mut self, values: Vec<Scalar>) -> Result<Scalar, CoreError> {
        self.function(
            values
                .into_iter()
                .enumerate()
                .map(|(index, value)| {
                    Ok((
                        Scalar::Int(
                            i32::try_from(index + 1)
                                .map_err(|_| CoreError::protocol("Sequence too long"))?,
                        ),
                        value,
                    ))
                })
                .collect::<Result<_, CoreError>>()?,
        )
    }
    fn find_domain(&self, hash: u64, keys: &[Scalar]) -> Option<Arc<FunctionDomain>> {
        self.domains
            .get(&hash)
            .and_then(|domains| domains.iter().find(|domain| &*domain.keys == keys))
            .cloned()
            .or_else(|| self.base.and_then(|base| base.find_domain(hash, keys)))
    }
    fn share_domain(&mut self, domain: Arc<FunctionDomain>) -> Arc<FunctionDomain> {
        if let Some(existing) = self.find_domain(domain.hash, &domain.keys) {
            return existing;
        }
        self.domains
            .entry(domain.hash)
            .or_default()
            .push(Arc::clone(&domain));
        domain
    }
    fn function_domain(&mut self, keys: Vec<Scalar>) -> Arc<FunctionDomain> {
        let mut hasher = ValueHasher::default();
        keys.hash(&mut hasher);
        let hash = hasher.finish();
        if let Some(existing) = self.find_domain(hash, &keys) {
            return existing;
        }
        let depth = keys
            .iter()
            .map(|key| match key {
                Scalar::Ref(index) => self.depth(*index),
                _ => 0,
            })
            .max()
            .unwrap_or(0);
        self.share_domain(Arc::new(FunctionDomain {
            keys: keys.into_boxed_slice(),
            hash,
            depth,
            #[cfg(not(target_arch = "wasm32"))]
            root_owned: self.base.is_none(),
            #[cfg(not(target_arch = "wasm32"))]
            root: None,
        }))
    }
    fn find_entry_domain(
        &self,
        hash: u64,
        entries: &[(Scalar, Scalar)],
    ) -> Option<Arc<FunctionDomain>> {
        self.domains
            .get(&hash)
            .and_then(|domains| {
                domains.iter().find(|domain| {
                    domain.keys.len() == entries.len()
                        && domain
                            .keys
                            .iter()
                            .zip(entries)
                            .all(|(key, entry)| *key == entry.0)
                })
            })
            .cloned()
            .or_else(|| {
                self.base
                    .and_then(|base| base.find_entry_domain(hash, entries))
            })
    }
    fn make_function(&mut self, entries: Vec<(Scalar, Scalar)>) -> Function {
        let mut hash = ValueHasher::default();
        entries.len().hash(&mut hash);
        for entry in &entries {
            entry.0.hash(&mut hash);
        }
        let domain = self
            .find_entry_domain(hash.finish(), &entries)
            .unwrap_or_else(|| self.function_domain(entries.iter().map(|entry| entry.0).collect()));
        Function::from_entries(domain, entries)
    }
    pub fn function(&mut self, mut values: Vec<(Scalar, Scalar)>) -> Result<Scalar, CoreError> {
        values.sort_by(|left, right| self.total(left.0, right.0));
        for pair in values.windows(2) {
            self.comparable(pair[0].0, pair[1].0)?;
            if pair[0].0 == pair[1].0 {
                return Err(CoreError::protocol("Duplicate function key"));
            }
        }
        let function = self.make_function(values);
        self.intern(Cell::Function(function))
    }
    pub fn members(&self, value: Scalar) -> Result<Vec<Scalar>, CoreError> {
        match self.cell(value)? {
            Cell::Set(values) => Ok(values.clone()),
            _ => Err(CoreError::protocol("Expected a finite set")),
        }
    }
    pub fn fold_members(
        &mut self,
        value: Scalar,
        fold: &Expression,
    ) -> Result<FoldMembers, CoreError> {
        let members = self.members(value)?;
        let replacement = self.check_fold_order(&members, fold)?;
        Ok(FoldMembers {
            members,
            replacement,
        })
    }
    pub fn require_comparable_members(
        &mut self,
        value: Scalar,
        message: &'static str,
    ) -> Result<(), CoreError> {
        let Ok(Cell::Set(members)) = self.cell(value) else {
            return Ok(());
        };
        let members = members.clone();
        let kind = |value: Scalar| match value {
            Scalar::Int(_) => Some(0),
            Scalar::Bool(_) => Some(1),
            Scalar::Ref(_) => matches!(self.cell(value), Ok(Cell::String(_))).then_some(2),
        };
        if members.first().is_none_or(|first| {
            members
                .iter()
                .all(|member| kind(*member) == kind(*first) && kind(*first).is_some())
        }) {
            return Ok(());
        }
        for (index, left) in members.iter().enumerate() {
            for right in &members[index + 1..] {
                if let Err(error) = self.same_kinds(*left, *right) {
                    return Err(match error.code {
                        "budget" | "evaluation-budget" => error,
                        _ => CoreError::protocol(message),
                    });
                }
            }
        }
        Ok(())
    }
    fn same_kinds(&mut self, left: Scalar, right: Scalar) -> Result<(), CoreError> {
        self.tick()?;
        if left == right
            || matches!(
                (left, right),
                (Scalar::Int(_), Scalar::Int(_)) | (Scalar::Bool(_), Scalar::Bool(_))
            )
            || self.model_comparison(left, right)?
        {
            return Ok(());
        }
        if self.rank(left) != self.rank(right) {
            return Err(CoreError::protocol("Incomparable TLA values"));
        }
        let pairs: Vec<(Scalar, Scalar)> = match (self.cell(left), self.cell(right)) {
            (Ok(Cell::Set(left)), Ok(Cell::Set(right))) if left.len() == right.len() => {
                left.iter().copied().zip(right.iter().copied()).collect()
            }
            (Ok(Cell::Function(left)), Ok(Cell::Function(right))) if left.len() == right.len() => {
                left.iter()
                    .zip(right.iter())
                    .flat_map(|(left, right)| [(left.0, right.0), (left.1, right.1)])
                    .collect()
            }
            _ => vec![],
        };
        pairs
            .into_iter()
            .try_for_each(|(left, right)| self.same_kinds(left, right))
    }
    fn fold_independence(&self, fold: &Expression) -> OrderIndependence {
        match self.prepared {
            Some(prepared) => prepared.fold_independence(fold, self.definitions),
            None => match fold {
                Expression::Fold {
                    accumulator, body, ..
                } => classify(*accumulator, body, self.definitions),
                _ => OrderIndependence::Dependent,
            },
        }
    }
    fn check_fold_order(
        &mut self,
        values: &[Scalar],
        fold: &Expression,
    ) -> Result<Option<&'static str>, CoreError> {
        let single = values.len() <= 1;
        let mut text = None;
        let mut independence = OrderIndependence::Dependent;
        let mut pending: Vec<_> = values.iter().rev().map(|value| (*value, false)).collect();
        let mut visited = HashSet::new();
        let mut keys = None;
        while let Some((value, is_key)) = pending.pop() {
            if is_key {
                let Scalar::Int(key) = value else {
                    return self.refuse_fold_order(single, text, unsupported_fold_order());
                };
                keys = Some(match keys {
                    Some((minimum, maximum)) => (key.min(minimum), key.max(maximum)),
                    None => (key, key),
                });
                continue;
            }
            if let Scalar::Ref(index) = value {
                if !visited.insert(index) {
                    continue;
                }
                self.tick()?;
                match self.cell(value)? {
                    Cell::ModelValue(_) => {
                        return self.refuse_fold_order(
                            single,
                            text,
                            CoreError::protocol(
                                "Unsupported TLA input: set fold order for model values is not implemented",
                            ),
                        );
                    }
                    Cell::String(_) => {
                        if text.is_none() {
                            if single {
                                return Ok(None);
                            }
                            independence = self.text_fold_order(fold, STRING_FOLD_ORDER)?;
                            text = Some(STRING_FOLD_ORDER);
                        }
                    }
                    Cell::Set(values) => {
                        pending.extend(values.iter().rev().map(|value| (*value, false)));
                    }
                    Cell::Function(entries) => {
                        let record = !entries.is_empty()
                            && entries
                                .iter()
                                .all(|entry| matches!(self.cell(entry.0), Ok(Cell::String(_))));
                        if record && text.is_none() {
                            if single {
                                return Ok(None);
                            }
                            independence = self.text_fold_order(fold, RECORD_FOLD_ORDER)?;
                            text = Some(RECORD_FOLD_ORDER);
                        }
                        for (key, value) in entries.iter().rev() {
                            pending.push((value, false));
                            if !record {
                                pending.push((key, true));
                            }
                        }
                    }
                }
            }
        }
        if let Some((minimum, maximum)) = keys
            && maximum.abs_diff(minimum) > i32::MAX as u32
        {
            return self.refuse_fold_order(
                single,
                text,
                CoreError::protocol(
                    "Unsupported TLA input: set fold order for integer function domains spanning more than i32::MAX",
                ),
            );
        }
        Ok((independence == OrderIndependence::Union)
            .then_some(text)
            .flatten())
    }
    fn text_fold_order(
        &self,
        fold: &Expression,
        message: &'static str,
    ) -> Result<OrderIndependence, CoreError> {
        match self.fold_independence(fold) {
            OrderIndependence::Dependent => Err(CoreError::protocol(message)),
            independence => Ok(independence),
        }
    }
    fn refuse_fold_order(
        &self,
        single: bool,
        text: Option<&'static str>,
        error: CoreError,
    ) -> Result<Option<&'static str>, CoreError> {
        if single {
            Ok(None)
        } else {
            Err(text.map_or(error, CoreError::protocol))
        }
    }
    pub fn contains(&self, value: Scalar, set: Scalar) -> Result<bool, CoreError> {
        let Cell::Set(values) = self.cell(set)? else {
            return Err(CoreError::protocol("Membership requires a finite set"));
        };
        for candidate in values {
            if self.equal(value, *candidate)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn select(&self, function: Scalar, key: Scalar) -> Result<Option<Scalar>, CoreError> {
        let Cell::Function(values) = self.cell(function)? else {
            return Err(CoreError::protocol("Expected a function"));
        };
        Ok(values.position(key).map(|position| values.value(position)))
    }
    pub fn at(&self, function: Scalar, key: Scalar) -> Result<Scalar, CoreError> {
        self.select(function, key)?
            .ok_or_else(|| CoreError::protocol("Function argument is outside its domain"))
    }
    pub fn domain(&mut self, function: Scalar) -> Result<Scalar, CoreError> {
        let Cell::Function(values) = self.cell(function)? else {
            return Err(CoreError::protocol("DOMAIN requires a function"));
        };
        self.set(values.domain.keys.to_vec())
    }
    pub fn entries(&self, function: Scalar) -> Result<Vec<(Scalar, Scalar)>, CoreError> {
        match self.cell(function)? {
            Cell::Function(values) => Ok(values.iter().collect()),
            _ => Err(CoreError::protocol("Expected a function")),
        }
    }
    pub fn scalar_record_shape(&self, value: Scalar) -> Result<Vec<(Scalar, bool)>, CoreError> {
        let Cell::Function(function) = self.cell(value)? else {
            return Err(CoreError::protocol(
                "Unsupported symmetry: expected a flat scalar record",
            ));
        };
        function
            .iter()
            .map(|(key, value)| {
                if !matches!(self.cell(key)?, Cell::String(_)) {
                    return Err(CoreError::protocol(
                        "Unsupported symmetry: record fields must be strings",
                    ));
                }
                match value {
                    Scalar::Int(_) => Ok((key, false)),
                    Scalar::Bool(_) => Ok((key, true)),
                    Scalar::Ref(_) => Err(CoreError::protocol(
                        "Unsupported symmetry: record fields must be Int/Bool",
                    )),
                }
            })
            .collect()
    }
    pub fn function_values(&self, function: Scalar) -> Result<FunctionValues, CoreError> {
        match self.cell(function)? {
            Cell::Function(function) => Ok(function.values()),
            _ => Err(CoreError::protocol("Expected a function")),
        }
    }
    pub fn binary_function_membership(&self, function: Scalar, range: Scalar) -> Option<usize> {
        #[cfg(not(target_arch = "wasm32"))]
        if self.budget.is_some() {
            return None;
        }
        let (Ok(Cell::Function(function)), Ok(Cell::Set(range))) =
            (self.cell(function), self.cell(range))
        else {
            return None;
        };
        match (function.binary_kind(), range.as_slice()) {
            (Some(false), [Scalar::Bool(false), Scalar::Bool(true)])
            | (Some(true), [Scalar::Int(0), Scalar::Int(1)]) => Some(function.len()),
            _ => None,
        }
    }
    pub fn check_tuple(&self, value: Scalar, size: Option<usize>) -> Result<(), CoreError> {
        if let Some(size) = size
            && self.sequence(value)?.len() != size
        {
            return Err(CoreError::protocol(
                "Bound tuple pattern has the wrong length",
            ));
        }
        Ok(())
    }
    pub fn sequence(&self, value: Scalar) -> Result<Vec<Scalar>, CoreError> {
        let values = self.entries(value)?;
        if !values
            .iter()
            .enumerate()
            .all(|(index, entry)| entry.0 == Scalar::Int((index + 1) as i32))
        {
            return Err(CoreError::protocol("Expected a sequence"));
        }
        Ok(values.into_iter().map(|entry| entry.1).collect())
    }
    fn update_domain(&self, domain: &Arc<FunctionDomain>) -> Arc<FunctionDomain> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let root = domain.root.as_ref().unwrap_or(domain);
            if let Some(alias) = self.aliases.iter().find(|alias| {
                alias
                    .root
                    .as_ref()
                    .is_some_and(|owner| Arc::ptr_eq(owner, root))
            }) {
                return Arc::clone(alias);
            }
        }
        Arc::clone(domain)
    }
    pub fn replace(
        &mut self,
        function: Scalar,
        path: &[Scalar],
        value: Scalar,
    ) -> Result<Scalar, CoreError> {
        let Some((first, rest)) = path.split_first() else {
            return Ok(value);
        };
        let Cell::Function(entries) = self.cell(function)? else {
            return Err(CoreError::protocol("Expected a function"));
        };
        let Some(position) = entries.position(*first) else {
            return Ok(function);
        };
        let previous = entries.value(position);
        let updated = self.replace(previous, rest, value)?;
        if updated == previous {
            return Ok(function);
        }
        let Cell::Function(entries) = self.cell(function)? else {
            unreachable!()
        };
        if let Some(probe) = entries.scalar_update(position, updated) {
            let mut hasher = ValueHasher::default();
            std::mem::discriminant(self.cell(function)?).hash(&mut hasher);
            probe.hash(&mut hasher);
            let hash = hasher.finish();
            if let Some(index) = self.find_updated(hash, &probe) {
                return Ok(Scalar::Ref(index));
            }
            let updated = probe.materialize(self.update_domain(&entries.domain));
            return self.insert_new(Cell::Function(updated), hash, true);
        }
        let updated = entries.updated(position, updated, self.update_domain(&entries.domain));
        self.intern(Cell::Function(updated))
    }
    pub fn path(&self, mut function: Scalar, path: &[Scalar]) -> Result<Option<Scalar>, CoreError> {
        for key in path {
            let Some(value) = self.select(function, *key)? else {
                return Ok(None);
            };
            function = value;
        }
        Ok(Some(function))
    }

    pub fn portable_bounded(
        &self,
        value: Scalar,
        remaining: &mut usize,
    ) -> Result<Option<Value>, CoreError> {
        fn size(
            arena: &Arena<'_>,
            value: Scalar,
            maximum: usize,
            memo: &mut HashMap<Scalar, usize>,
        ) -> Result<usize, CoreError> {
            if matches!(value, Scalar::Int(_) | Scalar::Bool(_)) {
                return Ok(64);
            }
            if let Some(size) = memo.get(&value) {
                return Ok(*size);
            }
            let mut total = 64usize;
            match value {
                Scalar::Int(_) | Scalar::Bool(_) => {}
                _ => match arena.cell(value)? {
                    Cell::String(text) | Cell::ModelValue(text) => {
                        total = total.saturating_add(text.len().saturating_mul(6))
                    }
                    Cell::Set(values) => {
                        for child in values {
                            total = total.saturating_add(size(arena, *child, maximum, memo)?);
                            if total > maximum {
                                break;
                            }
                        }
                    }
                    Cell::Function(values) => {
                        for (key, child) in values.iter() {
                            total = total.saturating_add(size(arena, key, maximum, memo)?);
                            total = total.saturating_add(size(arena, child, maximum, memo)?);
                            if total > maximum {
                                break;
                            }
                        }
                    }
                },
            }
            memo.insert(value, total);
            Ok(total)
        }
        let cost = size(self, value, *remaining, &mut HashMap::new())?;
        if cost > *remaining {
            return Ok(None);
        }
        *remaining -= cost;
        self.portable(value).map(Some)
    }

    pub fn portable(&self, value: Scalar) -> Result<Value, CoreError> {
        Ok(match value {
            Scalar::Int(value) => Value::Int(value.to_string()),
            Scalar::Bool(value) => Value::Bool(value),
            _ => match self.cell(value)? {
                Cell::String(value) => Value::String(value.clone()),
                Cell::ModelValue(value) => Value::ModelValue(value.clone()),
                Cell::Set(values) => Value::Set(
                    values
                        .iter()
                        .map(|value| self.portable(*value))
                        .collect::<Result<_, _>>()?,
                )
                .normalize()?,
                Cell::Function(values) => {
                    if values
                        .iter()
                        .enumerate()
                        .all(|(index, entry)| entry.0 == Scalar::Int((index + 1) as i32))
                    {
                        Value::List(
                            values
                                .iter()
                                .map(|entry| self.portable(entry.1))
                                .collect::<Result<_, _>>()?,
                        )
                    } else if values
                        .iter()
                        .all(|entry| matches!(self.cell(entry.0), Ok(Cell::String(_))))
                    {
                        Value::Record(
                            values
                                .iter()
                                .map(|entry| {
                                    let Cell::String(name) = self.cell(entry.0)? else {
                                        unreachable!()
                                    };
                                    Ok((name.clone(), self.portable(entry.1)?))
                                })
                                .collect::<Result<_, CoreError>>()?,
                        )
                        .normalize()?
                    } else {
                        Value::Map(
                            values
                                .iter()
                                .map(|entry| Ok((self.portable(entry.0)?, self.portable(entry.1)?)))
                                .collect::<Result<_, CoreError>>()?,
                        )
                        .normalize()?
                    }
                }
            },
        })
    }
}

const STRING_FOLD_ORDER: &str =
    "Unsupported TLA input: set fold order for strings requires TLC intern IDs absent from the IR";
const RECORD_FOLD_ORDER: &str = "Unsupported TLA input: set fold order for records requires TLC field-name intern IDs absent from the IR";

#[derive(Debug)]
pub(crate) struct FoldMembers {
    pub members: Vec<Scalar>,
    pub replacement: Option<&'static str>,
}

fn unsupported_fold_order() -> CoreError {
    CoreError::protocol("Unsupported TLA input: set fold order for strings, records or functions")
}

fn work_exhausted() -> CoreError {
    CoreError {
        code: "budget",
        message: "Safety work budget exhausted".into(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct ValueDelta {
    cells: Vec<Cell>,
    constants: Vec<Option<Scalar>>,
    data: HashMap<Value, Scalar>,
    types: HashMap<Expression, Option<Arc<MembershipSet>>>,
    registered: RegisteredMembership,
    membership: MembershipCache,
    offset: usize,
    pub work: u32,
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct ValueRemap {
    offset: usize,
    values: Vec<Scalar>,
}

#[cfg(not(target_arch = "wasm32"))]
impl ValueRemap {
    pub fn get(&self, value: Scalar) -> Scalar {
        match value {
            Scalar::Ref(index) if index as usize >= self.offset => {
                self.values[index as usize - self.offset]
            }
            _ => value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Arena, Cell, ValueHasher};
    use crate::ir::{Expression, FoldOrder, Scalar};
    use std::hash::{Hash, Hasher};

    #[test]
    fn fold_order_visits_shared_cells_once_per_call_and_obeys_the_work_budget() {
        let mut arena = Arena::new(150);
        let mut deep = Scalar::Int(0);
        for _ in 0..48 {
            deep = arena.tuple(vec![deep, deep]).unwrap();
        }
        let first = arena.tuple(vec![Scalar::Int(1), deep]).unwrap();
        let second = arena.tuple(vec![Scalar::Int(2), deep]).unwrap();
        let members = arena.set(vec![first, second]).unwrap();
        let before = arena.work;
        let fold = Expression::Fold {
            accumulator: 0,
            initial: Box::new(Expression::Old),
            collection: Box::new(Expression::Old),
            order: FoldOrder::SetMembers,
            body: Box::new(Expression::Old),
        };
        let result = arena.fold_members(members, &fold);
        assert_eq!(arena.work - before, 50);
        assert_eq!(result.unwrap().members, vec![first, second]);
        let error = arena.fold_members(members, &fold).unwrap_err();
        assert_eq!(error.code, "budget");
        assert_eq!(error.message, "Safety work budget exhausted");
        assert_eq!(arena.work, 150);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn worker_values_share_the_base_and_remap_conflicting_local_references() {
        use crate::safety_work::WorkBudget;
        let mut base = Arena::new(1000);
        let original = base.tuple(vec![Scalar::Int(0)]).unwrap();
        let budget = WorkBudget::new(900, 2);
        let mut first = base.fork(&budget, 0);
        let mut second = base.fork(&budget, 1);
        assert_eq!(first.tuple(vec![Scalar::Int(0)]).unwrap(), original);
        let left = first.set(vec![Scalar::Int(1)]).unwrap();
        let right = second.set(vec![Scalar::Int(2)]).unwrap();
        assert_eq!(left, right);
        let nested = second.tuple(vec![right, original]).unwrap();
        let first = first.into_delta();
        let second = second.into_delta();
        let first = base.merge(first).unwrap();
        let second = base.merge(second).unwrap();
        assert_ne!(first.get(left), second.get(right));
        assert_eq!(
            base.portable(first.get(left)).unwrap(),
            crate::Value::Set(vec![crate::Value::Int("1".into())])
        );
        assert_eq!(
            base.portable(second.get(right)).unwrap(),
            crate::Value::Set(vec![crate::Value::Int("2".into())])
        );
        let members = base.sequence(second.get(nested)).unwrap();
        assert_eq!(members, vec![second.get(right), original]);
        let canonical = base.set(vec![Scalar::Int(2)]).unwrap();
        assert_eq!(canonical, second.get(right));
    }

    #[test]
    fn colliding_function_hashes_keep_distinct_values() {
        let first = vec![
            (Scalar::Int(0), Scalar::Int(-1_370_170_012)),
            (Scalar::Int(95_947_124), Scalar::Int(0)),
        ];
        let second = vec![
            (Scalar::Int(0), Scalar::Int(803_057_848)),
            (Scalar::Int(1_091_714_232), Scalar::Int(-1_935_938_920)),
        ];
        let hash = |entries: Vec<(Scalar, Scalar)>| {
            let mut hasher = ValueHasher::default();
            Cell::Function(Arena::new(100).make_function(entries)).hash(&mut hasher);
            hasher.finish()
        };
        assert_eq!(hash(first.clone()), hash(second.clone()));
        let mut arena = Arena::new(100);
        let first_id = arena.function(first.clone()).unwrap();
        let second_id = arena.function(second.clone()).unwrap();
        assert_ne!(first_id, second_id);
        assert_eq!(arena.function(first.clone()).unwrap(), first_id);
        assert_eq!(arena.function(second.clone()).unwrap(), second_id);
        assert_eq!(arena.entries(first_id).unwrap(), first);
        assert_eq!(arena.entries(second_id).unwrap(), second);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn collisions_in_an_overlay_keep_base_and_local_values_distinct() {
        use crate::safety_work::WorkBudget;
        let first = vec![
            (Scalar::Int(0), Scalar::Int(-1_370_170_012)),
            (Scalar::Int(95_947_124), Scalar::Int(0)),
        ];
        let second = vec![
            (Scalar::Int(0), Scalar::Int(803_057_848)),
            (Scalar::Int(1_091_714_232), Scalar::Int(-1_935_938_920)),
        ];
        let mut arena = Arena::new(1000);
        let base = arena.function(first.clone()).unwrap();
        let budget = WorkBudget::new(900, 1);
        let mut overlay = arena.fork(&budget, 0);
        let local = overlay.function(second.clone()).unwrap();
        assert_ne!(base, local);
        assert_eq!(overlay.function(first.clone()).unwrap(), base);
        let mapping = arena.merge(overlay.into_delta()).unwrap();
        assert_eq!(arena.entries(base).unwrap(), first);
        assert_eq!(arena.entries(mapping.get(local)).unwrap(), second);
        assert_ne!(base, mapping.get(local));
        assert_eq!(arena.function(second).unwrap(), mapping.get(local));
    }

    #[test]
    fn replacement_keeps_the_normalized_domain_and_canonical_identity() {
        let mut arena = Arena::new(1000);
        let original = arena
            .function(vec![
                (Scalar::Int(3), Scalar::Int(30)),
                (Scalar::Int(1), Scalar::Int(10)),
                (Scalar::Int(2), Scalar::Int(20)),
            ])
            .unwrap();
        let updated = arena
            .replace(original, &[Scalar::Int(2)], Scalar::Int(21))
            .unwrap();
        let expected = arena
            .function(vec![
                (Scalar::Int(2), Scalar::Int(21)),
                (Scalar::Int(1), Scalar::Int(10)),
                (Scalar::Int(3), Scalar::Int(30)),
            ])
            .unwrap();
        assert_eq!(updated, expected);
        assert_eq!(arena.at(original, Scalar::Int(2)).unwrap(), Scalar::Int(20));
        assert_eq!(
            arena
                .replace(original, &[Scalar::Int(4)], Scalar::Int(40))
                .unwrap(),
            original
        );
        assert_eq!(
            arena
                .replace(updated, &[Scalar::Int(2)], Scalar::Int(20))
                .unwrap(),
            original
        );
    }
    #[test]
    fn model_value_lookup_preserves_semantic_order_and_missing_keys() {
        for names in [["a", "b", "c"], ["c", "b", "a"]] {
            let mut arena = Arena::new(1000);
            let keys: Vec<_> = names
                .iter()
                .map(|name| {
                    arena
                        .data(&crate::Value::ModelValue((*name).into()))
                        .unwrap()
                })
                .collect();
            let function = arena
                .function(
                    keys.iter()
                        .enumerate()
                        .map(|(index, key)| (*key, Scalar::Int(index as i32 + 10)))
                        .collect(),
                )
                .unwrap();
            for (index, key) in keys.into_iter().enumerate() {
                assert_eq!(
                    arena.at(function, key).unwrap(),
                    Scalar::Int(index as i32 + 10)
                );
            }
            let missing = arena
                .data(&crate::Value::ModelValue("missing".into()))
                .unwrap();
            assert_eq!(arena.select(function, missing).unwrap(), None);
            assert!(arena.at(function, missing).is_err());
            let string = arena.string("a".into()).unwrap();
            assert_eq!(arena.select(function, string).unwrap(), None);
            assert_eq!(arena.select(function, Scalar::Int(1)).unwrap(), None);
        }
    }

    #[test]
    fn sparse_model_value_domains_preserve_updates_and_missing_keys() {
        let mut arena = Arena::new(1000);
        let first = arena.data(&crate::Value::ModelValue("a".into())).unwrap();
        let missing = arena.data(&crate::Value::ModelValue("b".into())).unwrap();
        let last = arena.data(&crate::Value::ModelValue("c".into())).unwrap();
        let original = arena
            .function(vec![(last, Scalar::Int(30)), (first, Scalar::Int(10))])
            .unwrap();
        assert_eq!(arena.at(original, first).unwrap(), Scalar::Int(10));
        assert_eq!(arena.at(original, last).unwrap(), Scalar::Int(30));
        assert_eq!(arena.select(original, missing).unwrap(), None);
        assert!(arena.at(original, missing).is_err());
        assert_eq!(
            arena
                .replace(original, &[missing], Scalar::Int(99))
                .unwrap(),
            original
        );
        let updated = arena.replace(original, &[last], Scalar::Int(99)).unwrap();
        assert_eq!(arena.at(updated, last).unwrap(), Scalar::Int(99));
        assert_eq!(arena.at(updated, first).unwrap(), Scalar::Int(10));
        assert_eq!(arena.at(original, last).unwrap(), Scalar::Int(30));
    }

    #[test]
    fn integer_positions_preserve_sparse_domains_and_missing_keys() {
        let mut arena = Arena::new(1000);
        let dense = arena
            .function(
                [4, 1, 3, 2]
                    .into_iter()
                    .map(|key| (Scalar::Int(key), Scalar::Int(key * 10)))
                    .collect(),
            )
            .unwrap();
        for key in 1..=4 {
            assert_eq!(
                arena.at(dense, Scalar::Int(key)).unwrap(),
                Scalar::Int(key * 10)
            );
        }
        let sparse = arena
            .function(
                [-3, 0, 2, 10]
                    .into_iter()
                    .map(|key| (Scalar::Int(key), Scalar::Int(key * 10)))
                    .collect(),
            )
            .unwrap();
        for key in [-3, 0, 2, 10] {
            assert_eq!(
                arena.at(sparse, Scalar::Int(key)).unwrap(),
                Scalar::Int(key * 10)
            );
        }
        for key in [i32::MIN, -1, 1, 3, i32::MAX] {
            assert_eq!(arena.select(sparse, Scalar::Int(key)).unwrap(), None);
        }
        let updated = arena
            .replace(sparse, &[Scalar::Int(2)], Scalar::Int(77))
            .unwrap();
        assert_eq!(arena.at(updated, Scalar::Int(2)).unwrap(), Scalar::Int(77));
        assert_eq!(arena.at(updated, Scalar::Int(0)).unwrap(), Scalar::Int(0));
        assert_eq!(arena.at(sparse, Scalar::Int(2)).unwrap(), Scalar::Int(20));
        assert_eq!(
            arena
                .replace(updated, &[Scalar::Int(3)], Scalar::Int(99))
                .unwrap(),
            updated
        );
        let extreme = arena
            .function(vec![
                (Scalar::Int(i32::MIN), Scalar::Int(7)),
                (Scalar::Int(i32::MAX), Scalar::Int(9)),
            ])
            .unwrap();
        assert_eq!(
            arena.at(extreme, Scalar::Int(i32::MIN)).unwrap(),
            Scalar::Int(7)
        );
        assert_eq!(
            arena.at(extreme, Scalar::Int(i32::MAX)).unwrap(),
            Scalar::Int(9)
        );
        let string = arena.string("1".into()).unwrap();
        let record = arena.function(vec![(string, Scalar::Bool(true))]).unwrap();
        assert_eq!(arena.at(record, string).unwrap(), Scalar::Bool(true));
        assert_eq!(arena.select(record, Scalar::Int(1)).unwrap(), None);
        assert!(arena.at(record, Scalar::Int(1)).is_err());
        let boolean = arena
            .function(vec![
                (Scalar::Bool(false), Scalar::Int(3)),
                (Scalar::Bool(true), Scalar::Int(4)),
            ])
            .unwrap();
        assert_eq!(
            arena.at(boolean, Scalar::Bool(true)).unwrap(),
            Scalar::Int(4)
        );
        assert_eq!(arena.select(boolean, Scalar::Int(1)).unwrap(), None);
    }
}

#[cfg(test)]
mod compact_function_tests {
    use super::{Arena, Cell};
    use crate::ir::Scalar;
    use std::sync::Arc;

    #[test]
    fn updates_share_immutable_domains_and_preserve_typed_canonical_values() {
        let mut arena = Arena::new(100000);
        for width in [1, 16, 32, 64, 65] {
            for original in [Scalar::Bool(false), Scalar::Int(0)] {
                let base = arena.tuple(vec![original; width]).unwrap();
                let replacement = match original {
                    Scalar::Bool(_) => Scalar::Bool(true),
                    _ => Scalar::Int(1),
                };
                let changed = arena
                    .replace(base, &[Scalar::Int(width as i32)], replacement)
                    .unwrap();
                let Cell::Function(before) = arena.cell(base).unwrap() else {
                    panic!("function")
                };
                let Cell::Function(after) = arena.cell(changed).unwrap() else {
                    panic!("function")
                };
                assert!(Arc::ptr_eq(&before.domain, &after.domain));
                assert_eq!(arena.at(base, Scalar::Int(width as i32)).unwrap(), original);
                assert_eq!(
                    arena.at(changed, Scalar::Int(width as i32)).unwrap(),
                    replacement
                );
                assert_eq!(
                    arena
                        .replace(changed, &[Scalar::Int(width as i32)], original)
                        .unwrap(),
                    base
                );
                let general = arena
                    .replace(changed, &[Scalar::Int(1)], Scalar::Int(-17))
                    .unwrap();
                assert_eq!(arena.at(general, Scalar::Int(1)).unwrap(), Scalar::Int(-17));
                let restored = arena
                    .replace(
                        general,
                        &[Scalar::Int(1)],
                        if width == 1 { replacement } else { original },
                    )
                    .unwrap();
                assert_eq!(restored, changed);
            }
        }
        let integer = arena.tuple(vec![Scalar::Int(0); 64]).unwrap();
        let boolean = arena.tuple(vec![Scalar::Bool(false); 64]).unwrap();
        assert_ne!(integer, boolean);
        assert!(arena.equal(integer, boolean).is_err());
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod compact_overlay_tests {
    use super::Arena;
    use crate::ir::Scalar;
    use crate::safety_work::WorkBudget;

    #[test]
    fn overlays_remap_function_keys_and_payloads_without_mutating_the_base() {
        let mut base = Arena::new(10000);
        let original = base.tuple(vec![Scalar::Int(0); 64]).unwrap();
        let budget = WorkBudget::new(9000, 2);
        let mut first = base.fork(&budget, 0);
        let mut second = base.fork(&budget, 1);
        let left = first.string("left".into()).unwrap();
        let right = second.string("right".into()).unwrap();
        assert_eq!(left, right);
        let changed = second
            .replace(original, &[Scalar::Int(64)], Scalar::Int(1))
            .unwrap();
        let keyed = second.function(vec![(right, changed)]).unwrap();
        let first = first.into_delta();
        let second = second.into_delta();
        let first_map = base.merge(first).unwrap();
        let second_map = base.merge(second).unwrap();
        assert_ne!(first_map.get(left), second_map.get(right));
        let mapped = second_map.get(keyed);
        let key = second_map.get(right);
        assert_eq!(base.at(mapped, key).unwrap(), second_map.get(changed));
        assert_eq!(base.at(original, Scalar::Int(64)).unwrap(), Scalar::Int(0));
        assert_eq!(
            base.at(second_map.get(changed), Scalar::Int(64)).unwrap(),
            Scalar::Int(1)
        );
        assert_eq!(
            base.function(vec![(key, second_map.get(changed))]).unwrap(),
            mapped
        );
        assert!(base.portable(mapped).is_ok());
    }
}

#[cfg(test)]
mod compact_domain_collision_tests {
    use super::{Arena, Cell};
    use crate::ir::Scalar;

    #[test]
    fn packed_functions_compare_domains_after_a_domain_hash_collision() {
        let mut arena = Arena::new(100);
        let left = [30050, 14021977, 1972786496]
            .map(|key| (Scalar::Int(key), Scalar::Bool(false)))
            .to_vec();
        let right = [3176, 1115303, 708440656]
            .map(|key| (Scalar::Int(key), Scalar::Bool(false)))
            .to_vec();
        let first = arena.function(left.clone()).unwrap();
        let second = arena.function(right.clone()).unwrap();
        let Cell::Function(first_value) = arena.cell(first).unwrap() else {
            panic!("function")
        };
        let Cell::Function(second_value) = arena.cell(second).unwrap() else {
            panic!("function")
        };
        assert_eq!(first_value.domain.hash, second_value.domain.hash);
        assert_ne!(first, second);
        assert!(!arena.equal(first, second).unwrap());
        assert_eq!(arena.function(left).unwrap(), first);
        assert_eq!(arena.function(right).unwrap(), second);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod compact_domain_owner_tests {
    use super::{Arena, Cell};
    use crate::ir::Scalar;
    use crate::safety_work::WorkBudget;
    use std::sync::Arc;

    #[test]
    fn a_worker_created_domain_is_registered_in_the_root_and_reused() {
        let mut root = Arena::new(1000);
        let budget = WorkBudget::new(900, 1);
        let mut worker = root.fork(&budget, 0);
        let original = worker.tuple(vec![Scalar::Int(0); 32]).unwrap();
        let Cell::Function(function) = worker.cell(original).unwrap() else {
            panic!("function")
        };
        assert!(!function.domain.root_owned);
        let delta = worker.into_delta();
        let mapping = root.merge(delta).unwrap();
        let value = mapping.get(original);
        let Cell::Function(function) = root.cell(value).unwrap() else {
            panic!("function")
        };
        assert!(function.domain.root_owned);
        let domain = Arc::clone(&function.domain);
        let changed = root
            .replace(value, &[Scalar::Int(32)], Scalar::Int(1))
            .unwrap();
        let Cell::Function(function) = root.cell(changed).unwrap() else {
            panic!("function")
        };
        assert!(Arc::ptr_eq(&domain, &function.domain));
        assert_eq!(root.tuple(vec![Scalar::Int(0); 32]).unwrap(), value);
    }
}

#[cfg(test)]
mod binary_membership_tests {
    use super::Arena;
    use crate::ir::Scalar;

    #[test]
    fn bulk_membership_requires_the_exact_type_range_and_packed_width() {
        let mut arena = Arena::new(1000);
        let integers = arena.tuple(vec![Scalar::Int(0); 64]).unwrap();
        let booleans = arena.tuple(vec![Scalar::Bool(false); 64]).unwrap();
        let wide = arena.tuple(vec![Scalar::Int(0); 65]).unwrap();
        let mixed = arena
            .tuple(vec![Scalar::Bool(false), Scalar::Int(0)])
            .unwrap();
        let int_range = arena.set(vec![Scalar::Int(0), Scalar::Int(1)]).unwrap();
        let bool_range = arena
            .set(vec![Scalar::Bool(false), Scalar::Bool(true)])
            .unwrap();
        let narrow = arena.set(vec![Scalar::Int(0)]).unwrap();
        assert_eq!(
            arena.binary_function_membership(integers, int_range),
            Some(64)
        );
        assert_eq!(
            arena.binary_function_membership(booleans, bool_range),
            Some(64)
        );
        for (function, range) in [
            (integers, bool_range),
            (booleans, int_range),
            (integers, narrow),
            (wide, int_range),
            (mixed, int_range),
        ] {
            assert_eq!(arena.binary_function_membership(function, range), None);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let budget = crate::safety_work::WorkBudget::new(900, 1);
            let worker = arena.fork(&budget, 0);
            assert_eq!(worker.binary_function_membership(integers, int_range), None);
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod compact_worker_alias_tests {
    use super::{Arena, Cell};
    use crate::ir::Scalar;
    use crate::safety_work::WorkBudget;
    use std::sync::Arc;

    #[test]
    fn worker_updates_use_private_counters_and_merge_back_to_the_shared_root() {
        let mut root = Arena::new(10000);
        let original = root.tuple(vec![Scalar::Int(0); 64]).unwrap();
        let Cell::Function(function) = root.cell(original).unwrap() else {
            panic!("function")
        };
        let original_domain = Arc::clone(&function.domain);
        let budget = WorkBudget::new(9000, 2);
        let mut first = root.fork(&budget, 0);
        let mut second = root.fork(&budget, 1);
        let left = first
            .replace(original, &[Scalar::Int(1)], Scalar::Int(1))
            .unwrap();
        let right = second
            .replace(original, &[Scalar::Int(64)], Scalar::Int(1))
            .unwrap();
        let Cell::Function(left_value) = first.cell(left).unwrap() else {
            panic!("function")
        };
        let Cell::Function(right_value) = second.cell(right).unwrap() else {
            panic!("function")
        };
        assert!(!Arc::ptr_eq(&left_value.domain, &right_value.domain));
        assert!(Arc::ptr_eq(
            left_value.domain.root.as_ref().unwrap(),
            &original_domain
        ));
        assert!(Arc::ptr_eq(
            right_value.domain.root.as_ref().unwrap(),
            &original_domain
        ));
        let first = first.into_delta();
        let second = second.into_delta();
        let left_map = root.merge(first).unwrap();
        let right_map = root.merge(second).unwrap();
        for value in [left_map.get(left), right_map.get(right)] {
            let Cell::Function(function) = root.cell(value).unwrap() else {
                panic!("function")
            };
            assert!(Arc::ptr_eq(&function.domain, &original_domain));
            assert!(function.domain.root.is_none());
        }
        assert_eq!(root.at(original, Scalar::Int(1)).unwrap(), Scalar::Int(0));
    }
}

#[cfg(test)]
mod function_probe_tests {
    use super::{Arena, Cell, ValueHasher};
    use crate::ir::Scalar;
    use std::hash::{Hash, Hasher};

    #[test]
    fn borrowed_updates_match_independently_constructed_values_and_hashes() {
        let mut arena = Arena::new(1000);
        let child = arena.set(vec![Scalar::Int(7)]).unwrap();
        for (original, replacement, width) in [
            (Scalar::Int(9), Scalar::Int(8), 1),
            (Scalar::Int(9), Scalar::Int(8), 2),
            (child, Scalar::Int(8), 7),
            (child, Scalar::Int(8), 64),
            (Scalar::Int(0), Scalar::Int(1), 65),
        ] {
            let base = arena.tuple(vec![original; width]).unwrap();
            for position in 0..width {
                let mut expected = vec![original; width];
                expected[position] = replacement;
                let expected = arena.tuple(expected).unwrap();
                let before = arena.cell(base).unwrap();
                let after = arena.cell(expected).unwrap();
                let (Cell::Function(before_function), Cell::Function(after_function)) =
                    (before, after)
                else {
                    panic!("function")
                };
                let probe = before_function
                    .scalar_update(position, replacement)
                    .unwrap();
                assert!(probe.matches(after_function));
                assert!(!probe.matches(before_function));
                let mut borrowed_hash = ValueHasher::default();
                std::mem::discriminant(before).hash(&mut borrowed_hash);
                probe.hash(&mut borrowed_hash);
                let mut stored_hash = ValueHasher::default();
                after.hash(&mut stored_hash);
                assert_eq!(borrowed_hash.finish(), stored_hash.finish());
                assert_eq!(
                    arena
                        .replace(base, &[Scalar::Int(position as i32 + 1)], replacement)
                        .unwrap(),
                    expected
                );
            }
        }
    }

    #[test]
    fn packed_values_and_repacking_keep_the_existing_canonical_path() {
        let mut arena = Arena::new(100);
        let values = arena.tuple(vec![Scalar::Int(9), Scalar::Int(0)]).unwrap();
        let Cell::Function(function) = arena.cell(values).unwrap() else {
            panic!("function")
        };
        assert!(function.scalar_update(0, Scalar::Int(1)).is_none());
        let expected = arena.tuple(vec![Scalar::Int(1), Scalar::Int(0)]).unwrap();
        assert_eq!(
            arena
                .replace(values, &[Scalar::Int(1)], Scalar::Int(1))
                .unwrap(),
            expected
        );
        let Cell::Function(function) = arena.cell(expected).unwrap() else {
            panic!("function")
        };
        assert!(function.scalar_update(0, Scalar::Int(9)).is_none());
        assert_eq!(
            arena
                .replace(expected, &[Scalar::Int(1)], Scalar::Int(9))
                .unwrap(),
            values
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn an_update_collision_in_the_base_does_not_replace_the_local_value() {
        use crate::safety_work::WorkBudget;
        let mut root = Arena::new(1000);
        let collision = root
            .function(vec![
                (Scalar::Int(0), Scalar::Int(803_057_848)),
                (Scalar::Int(1_091_714_232), Scalar::Int(-1_935_938_920)),
            ])
            .unwrap();
        let budget = WorkBudget::new(900, 1);
        let mut worker = root.fork(&budget, 0);
        let source = worker
            .function(vec![
                (Scalar::Int(0), Scalar::Int(2)),
                (Scalar::Int(95_947_124), Scalar::Int(0)),
            ])
            .unwrap();
        let updated = worker
            .replace(source, &[Scalar::Int(0)], Scalar::Int(-1_370_170_012))
            .unwrap();
        assert_ne!(updated, collision);
        assert_eq!(
            worker.at(updated, Scalar::Int(0)).unwrap(),
            Scalar::Int(-1_370_170_012)
        );
        assert_eq!(
            worker
                .replace(source, &[Scalar::Int(0)], Scalar::Int(-1_370_170_012))
                .unwrap(),
            updated
        );
        let delta = worker.into_delta();
        let mapping = root.merge(delta).unwrap();
        assert_ne!(mapping.get(updated), collision);
        assert_eq!(
            root.at(collision, Scalar::Int(0)).unwrap(),
            Scalar::Int(803_057_848)
        );
    }
}

#[cfg(test)]
mod packed_hash_tests {
    use super::{Arena, ValueHasher};
    use crate::ir::Scalar;
    use std::collections::HashSet;
    use std::hash::{Hash, Hasher};

    #[test]
    fn packed_cube_hashes_keep_all_values_and_spread_index_bits() {
        let mut arena = Arena::new(10000);
        let mut hashes = HashSet::new();
        let mut buckets = [0_u16; 256];
        for bits in 0..4096_u64 {
            let value = arena
                .tuple(
                    (0..12)
                        .map(|bit| Scalar::Int(((bits >> bit) & 1) as i32))
                        .collect(),
                )
                .unwrap();
            let mut hasher = ValueHasher::default();
            arena.cell(value).unwrap().hash(&mut hasher);
            let hash = hasher.finish();
            assert!(hashes.insert(hash));
            buckets[(hash & 255) as usize] += 1;
        }
        assert!(buckets.iter().all(|count| *count > 0 && *count <= 64));
        let zero = arena.tuple(vec![Scalar::Int(0); 64]).unwrap();
        let mut hashes = HashSet::new();
        let mut hasher = ValueHasher::default();
        arena.cell(zero).unwrap().hash(&mut hasher);
        hashes.insert(hasher.finish());
        for bit in 0..64 {
            let changed = arena
                .replace(zero, &[Scalar::Int(bit + 1)], Scalar::Int(1))
                .unwrap();
            let mut hasher = ValueHasher::default();
            arena.cell(changed).unwrap().hash(&mut hasher);
            assert!(hashes.insert(hasher.finish()));
        }
    }

    #[test]
    fn an_int_bool_hash_collision_preserves_type_and_canonical_identity() {
        let mut arena = Arena::new(100);
        let boolean = arena.tuple(vec![Scalar::Bool(false); 64]).unwrap();
        let integers = (0..64)
            .map(|bit| Scalar::Int(((0x9e37_79b9_7f4a_7c15_u64 >> bit) & 1) as i32))
            .collect::<Vec<_>>();
        let integer = arena.tuple(integers.clone()).unwrap();
        let mut boolean_hash = ValueHasher::default();
        arena.cell(boolean).unwrap().hash(&mut boolean_hash);
        let mut integer_hash = ValueHasher::default();
        arena.cell(integer).unwrap().hash(&mut integer_hash);
        assert_eq!(boolean_hash.finish(), integer_hash.finish());
        assert_ne!(boolean, integer);
        assert!(arena.equal(boolean, integer).is_err());
        assert_eq!(arena.tuple(vec![Scalar::Bool(false); 64]).unwrap(), boolean);
        assert_eq!(arena.tuple(integers).unwrap(), integer);
    }
}

#[cfg(test)]
mod prepared_data_tests {
    use super::Arena;
    use crate::Value;
    use crate::ir::{Expression, Scalar};
    use crate::safety_expressions::EvalContext;
    use crate::safety_prepared::PreparedProgram;

    #[test]
    fn constants_are_lazy_and_cached_only_after_success_in_each_arena() {
        let invalid = Expression::Data {
            value: Value::Int("invalid".into()),
        };
        let text = Expression::Data {
            value: Value::String("constant".into()),
        };
        let program = PreparedProgram::from_roots(vec![&invalid, &text]);
        let mut arena = Arena::with_prepared(100, &program);
        assert_eq!(arena.work, 0);
        for _ in 0..2 {
            let error = invalid
                .eval(EvalContext::new(&[], &[]), &mut arena)
                .unwrap_err();
            assert_eq!(error.code, "protocol");
            assert_eq!(arena.work, 0);
        }
        let value = text.eval(EvalContext::new(&[], &[]), &mut arena).unwrap();
        let work = arena.work;
        for _ in 0..8 {
            assert_eq!(
                text.eval(EvalContext::new(&[], &[]), &mut arena).unwrap(),
                value
            );
            assert_eq!(arena.work, work);
        }
        let mut other = Arena::with_prepared(100, &program);
        other.string("different".into()).unwrap();
        let other_value = text.eval(EvalContext::new(&[], &[]), &mut other).unwrap();
        assert_ne!(value, other_value);
        assert_eq!(
            arena.portable(value).unwrap(),
            other.portable(other_value).unwrap()
        );
    }

    #[test]
    fn untaken_data_and_unregistered_expressions_keep_interpreter_behavior() {
        let expression = Expression::If {
            condition: Box::new(Expression::Literal {
                value: Scalar::Bool(false),
            }),
            consequent: Box::new(Expression::Data {
                value: Value::Int("invalid".into()),
            }),
            alternative: Box::new(Expression::Data {
                value: Value::String("ok".into()),
            }),
        };
        let program = PreparedProgram::from_roots(vec![&expression]);
        let mut prepared = Arena::with_prepared(100, &program);
        let mut plain = Arena::new(100);
        let expected = expression
            .eval(EvalContext::new(&[], &[]), &mut plain)
            .unwrap();
        let actual = expression
            .eval(EvalContext::new(&[], &[]), &mut prepared)
            .unwrap();
        assert_eq!(
            plain.portable(expected).unwrap(),
            prepared.portable(actual).unwrap()
        );
        assert_eq!(plain.work, prepared.work);
        let dynamic = Expression::Data {
            value: Value::String("dynamic".into()),
        };
        let actual = dynamic
            .eval(EvalContext::new(&[], &[]), &mut prepared)
            .unwrap();
        assert_eq!(
            prepared.portable(actual).unwrap(),
            Value::String("dynamic".into())
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn cached_worker_references_are_remapped_when_merged() {
        use crate::safety_work::WorkBudget;
        let expression = Expression::Data {
            value: Value::String("right".into()),
        };
        let program = PreparedProgram::from_roots(vec![&expression]);
        let mut root = Arena::with_prepared(1000, &program);
        let budget = WorkBudget::new(900, 2);
        let mut first = root.fork(&budget, 0);
        let mut second = root.fork(&budget, 1);
        let left = first.string("left".into()).unwrap();
        let right = expression
            .eval(EvalContext::new(&[], &[]), &mut second)
            .unwrap();
        assert_eq!(left, right);
        let first = first.into_delta();
        let second = second.into_delta();
        let first_map = root.merge(first).unwrap();
        let second_map = root.merge(second).unwrap();
        let actual = expression
            .eval(EvalContext::new(&[], &[]), &mut root)
            .unwrap();
        assert_eq!(actual, second_map.get(right));
        assert_ne!(actual, first_map.get(left));
        assert_eq!(
            root.portable(actual).unwrap(),
            Value::String("right".into())
        );
    }
}
