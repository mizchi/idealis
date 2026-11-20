use roracle::Error;
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Serialize, Serializer};
use serde_json::Number;

#[derive(Debug)]
pub struct NodeInfo {
    pub offset: usize,
    pub height: u32,
    pub nodes: u32,
}
pub struct Tape<'a> {
    data: &'a [u8],
    nodes: Vec<NodeInfo>,
}
#[derive(Clone, Copy, Debug)]
pub struct View<'a> {
    data: &'a [u8],
    offset: usize,
}

fn invalid() -> Error {
    Error("invalid typed payload".into())
}
fn word(data: &[u8], offset: usize) -> Result<usize, Error> {
    Ok(u32::from_le_bytes(
        data.get(offset..offset + 4)
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?,
    ) as usize)
}
fn text(data: &[u8]) -> Result<&str, Error> {
    std::str::from_utf8(data).map_err(|_| invalid())
}

impl<'a> Tape<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, Error> {
        if data.is_empty() || data.len() > 64 * 1024 * 1024 {
            return Err(invalid());
        }
        let mut tape = Self {
            data,
            nodes: Vec::new(),
        };
        let mut offset = 0;
        while offset < data.len() {
            offset = tape.validate(offset, 0)?.0;
        }
        Ok(tape)
    }
    fn validate(&mut self, offset: usize, depth: u32) -> Result<(usize, u32, u32), Error> {
        if self.nodes.len() >= 1_000_000 || depth > 127 {
            return Err(invalid());
        }
        let length = word(self.data, offset)?;
        let end = offset
            .checked_add(length)
            .filter(|end| *end <= self.data.len())
            .ok_or_else(invalid)?;
        let body = self
            .data
            .get(offset + 4..end)
            .filter(|body| !body.is_empty())
            .ok_or_else(invalid)?;
        let tag = body[0];
        let index = self.nodes.len();
        self.nodes.push(NodeInfo {
            offset,
            height: 0,
            nodes: 1,
        });
        let mut height = 0;
        let mut nodes = 1u32;
        match tag {
            0..=2 if body.len() == 1 => {}
            3 => {
                let value = text(&body[1..])?;
                if value.trim() != value {
                    return Err(invalid());
                }
                let number: Number = value.parse().map_err(|_| invalid())?;
                if number.as_f64().is_none_or(|value| {
                    !value.is_finite() || (value == 0.0 && value.is_sign_negative())
                }) {
                    return Err(invalid());
                }
            }
            4 => {
                text(&body[1..])?;
            }
            5 | 6 => {
                let count = word(self.data, offset + 5)?;
                if count > 1_000_000 {
                    return Err(invalid());
                }
                let mut cursor = offset + 9;
                let mut previous: Option<&str> = None;
                for _ in 0..count {
                    if tag == 6 {
                        let size = word(self.data, cursor)?;
                        cursor += 4;
                        let key = text(
                            self.data
                                .get(cursor..cursor.checked_add(size).ok_or_else(invalid)?)
                                .ok_or_else(invalid)?,
                        )?;
                        if previous.is_some_and(|previous| {
                            previous.encode_utf16().cmp(key.encode_utf16()).is_ge()
                        }) {
                            return Err(invalid());
                        }
                        previous = Some(key);
                        cursor += size;
                        nodes = nodes.checked_add(1).ok_or_else(invalid)?;
                    }
                    if cursor >= end {
                        return Err(invalid());
                    }
                    let (next, child_height, child_nodes) = self.validate(cursor, depth + 1)?;
                    cursor = next;
                    height = height.max(child_height);
                    nodes = nodes
                        .checked_add(child_nodes)
                        .filter(|nodes| *nodes <= 1_000_000)
                        .ok_or_else(invalid)?;
                }
                if cursor != end {
                    return Err(invalid());
                }
                height += 1;
            }
            _ => return Err(invalid()),
        }
        self.nodes[index].height = height;
        self.nodes[index].nodes = nodes;
        Ok((end, height, nodes))
    }
    pub fn info(&self, offset: u32) -> Result<&NodeInfo, Error> {
        self.nodes
            .binary_search_by_key(&(offset as usize), |node| node.offset)
            .map(|index| &self.nodes[index])
            .map_err(|_| invalid())
    }
    pub fn view(&self, offset: u32) -> Result<View<'a>, Error> {
        self.info(offset)?;
        Ok(View {
            data: self.data,
            offset: offset as usize,
        })
    }
}

impl<'a> View<'a> {
    fn length(self) -> usize {
        word(self.data, self.offset).expect("validated length")
    }
    fn tag(self) -> u8 {
        self.data[self.offset + 4]
    }
    fn body(self) -> &'a [u8] {
        &self.data[self.offset + 5..self.offset + self.length()]
    }
    fn number(self) -> Number {
        text(self.body())
            .expect("validated UTF-8")
            .parse()
            .expect("validated number")
    }
    fn child(self, offset: usize) -> Self {
        Self {
            data: self.data,
            offset,
        }
    }
    fn key(self, cursor: &mut usize) -> &'a str {
        let length = word(self.data, *cursor).expect("validated key length");
        *cursor += 4;
        let value = text(&self.data[*cursor..*cursor + length]).expect("validated key");
        *cursor += length;
        value
    }
}

impl PartialEq for View<'_> {
    fn eq(&self, other: &Self) -> bool {
        if self.data[self.offset..self.offset + self.length()]
            == other.data[other.offset..other.offset + other.length()]
        {
            return true;
        }
        if self.tag() != other.tag() {
            return false;
        }
        match self.tag() {
            3 => self.number() == other.number(),
            5 | 6 => {
                let count = word(self.data, self.offset + 5).expect("validated count");
                if count != word(other.data, other.offset + 5).expect("validated count") {
                    return false;
                }
                let mut left = self.offset + 9;
                let mut right = other.offset + 9;
                for _ in 0..count {
                    if self.tag() == 6 && self.key(&mut left) != other.key(&mut right) {
                        return false;
                    }
                    let left_value = self.child(left);
                    let right_value = other.child(right);
                    if left_value != right_value {
                        return false;
                    }
                    left += left_value.length();
                    right += right_value.length();
                }
                true
            }
            _ => false,
        }
    }
}

impl Serialize for View<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.tag() {
            0 => serializer.serialize_none(),
            1 => serializer.serialize_bool(false),
            2 => serializer.serialize_bool(true),
            3 => self.number().serialize(serializer),
            4 => serializer.serialize_str(text(self.body()).expect("validated UTF-8")),
            5 => {
                let count = word(self.data, self.offset + 5).expect("validated count");
                let mut output = serializer.serialize_seq(Some(count))?;
                let mut cursor = self.offset + 9;
                for _ in 0..count {
                    let value = self.child(cursor);
                    output.serialize_element(&value)?;
                    cursor += value.length();
                }
                output.end()
            }
            6 => {
                let count = word(self.data, self.offset + 5).expect("validated count");
                let mut output = serializer.serialize_map(Some(count))?;
                let mut cursor = self.offset + 9;
                for _ in 0..count {
                    let key = self.key(&mut cursor);
                    let value = self.child(cursor);
                    output.serialize_entry(key, &value)?;
                    cursor += value.length();
                }
                output.end()
            }
            _ => unreachable!("validated tag"),
        }
    }
}
