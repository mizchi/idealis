use crate::{Compiler, ConfigValue, DefinitionKind, E, Json, Result, json, unsupported};
use std::collections::HashSet;

impl Compiler<'_> {
    pub(super) fn node_symmetry(&self) -> Result<Option<Json>> {
        let Some(name) = &self.config.symmetry else {
            return Ok(None);
        };
        if !self.extends("TLC")
            || self
                .module
                .constants
                .iter()
                .chain(&self.module.variables)
                .any(|declaration| declaration.name == "Permutations")
            || self
                .module
                .definitions
                .iter()
                .any(|definition| definition.name == "Permutations")
        {
            return unsupported("SYMMETRY requires TLC.Permutations");
        }
        let Some(definition) = self
            .module
            .definitions
            .iter()
            .find(|definition| definition.name == *name)
        else {
            return unsupported("undefined SYMMETRY operator");
        };
        if definition.is_recursive
            || !matches!(&definition.head, DefinitionKind::Operator { params } if params.is_empty())
        {
            return unsupported("SYMMETRY must be a zero-argument operator");
        }
        let E::Apply { operator, args } = &definition.body.node else {
            return unsupported("SYMMETRY requires Permutations(Nodes)");
        };
        if operator != "Permutations" || args.len() != 1 {
            return unsupported("SYMMETRY requires one complete permutation group");
        }
        let E::Identifier { name } = &args[0].node else {
            return unsupported("SYMMETRY requires a directly assigned constant set");
        };
        let Some(ConfigValue::Set { elements }) = self.constants.get(name) else {
            return unsupported("SYMMETRY requires a finite constant set");
        };
        if !(2..=64).contains(&elements.len()) {
            return unsupported("SYMMETRY requires 2 to 64 model values");
        }
        let mut names = Vec::new();
        for element in elements {
            let ConfigValue::ModelValue { name } = element else {
                return unsupported("SYMMETRY requires untyped model values");
            };
            if name.as_bytes().get(1) == Some(&b'_')
                || rtlc::Value::ModelValue(name.clone()).normalize().is_err()
            {
                return unsupported("SYMMETRY requires valid untyped model values");
            }
            names.push(name.clone());
        }
        if names.iter().collect::<HashSet<_>>().len() != names.len() {
            return unsupported("SYMMETRY model values must be distinct");
        }
        names.sort();
        Ok(Some(json!({"modelValues":names})))
    }
}
