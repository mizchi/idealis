use crate::batch::{Batch, Input, InputObservation};
use roracle::Error;

struct Reader<'a> {
    data: &'a [u8],
    cursor: usize,
}
impl<'a> Reader<'a> {
    fn word(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.bytes(4)?
                .try_into()
                .map_err(|_| Error("invalid word".into()))?,
        ))
    }
    fn bytes(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let end = self
            .cursor
            .checked_add(length)
            .filter(|end| *end <= self.data.len())
            .ok_or_else(|| Error("invalid buffer bounds".into()))?;
        let bytes = &self.data[self.cursor..end];
        self.cursor = end;
        Ok(bytes)
    }
    fn count(&mut self, limit: usize) -> Result<usize, Error> {
        let count = self.word()? as usize;
        if count > limit {
            return Err(Error("invalid buffer count".into()));
        }
        Ok(count)
    }
    fn text(&mut self) -> Result<&'a str, Error> {
        let length = self.count(16 * 1024 * 1024)?;
        std::str::from_utf8(self.bytes(length)?).map_err(|_| Error("invalid UTF-8".into()))
    }
    fn observation(&mut self) -> Result<InputObservation<'a>, Error> {
        Ok(match self.word()? {
            0 => InputObservation::Value(self.word()?),
            1 => {
                let count = self.count(1_000_000)?;
                InputObservation::Allowed(
                    (0..count).map(|_| self.word()).collect::<Result<_, _>>()?,
                )
            }
            2 => InputObservation::Unknown(self.text()?),
            3 => InputObservation::Invalid(self.text()?),
            4 => InputObservation::Error(self.text()?),
            _ => return Err(Error("invalid observation tag".into())),
        })
    }
}
fn parse(data: &[u8]) -> Result<Batch<'_>, Error> {
    if data.len() > 64 * 1024 * 1024 {
        return Err(Error("typed buffer exceeds 64 MiB".into()));
    }
    let mut reader = Reader { data, cursor: 0 };
    if reader.word()? != 0x42524f49 || reader.word()? != 1 {
        return Err(Error("invalid typed buffer ABI".into()));
    }
    let version = reader.word()?;
    let group = reader.text()?;
    let reference = reader.text()?;
    let count = reader.count(256)?;
    let implementations = (0..count)
        .map(|_| reader.text())
        .collect::<Result<_, _>>()?;
    let count = reader.count(4096)?;
    let mut cases = Vec::with_capacity(count);
    for _ in 0..count {
        let id = reader.text()?;
        let input = reader.word()?;
        let reference = reader.observation()?;
        let count = reader.count(256)?;
        let actual = (0..count)
            .map(|_| reader.observation())
            .collect::<Result<_, _>>()?;
        cases.push(Input {
            id,
            input,
            reference,
            actual,
        });
    }
    let length = reader.count(64 * 1024 * 1024)?;
    let payload = reader.bytes(length)?;
    if reader.cursor != data.len() {
        return Err(Error("trailing typed data".into()));
    }
    Ok(Batch {
        version,
        group,
        reference,
        implementations,
        cases,
        payload,
    })
}
pub fn dispatch(data: &[u8]) -> String {
    crate::batch::reply(parse(data).and_then(crate::batch::check))
}
