// Implements fixtures/rmodel/permissions.model against the generated trait
// and serves it to rcheck (plan 0105). The argument selects a fault: "read"
// skips the grant check, "close" leaves the document open.

///|
struct Domain {
  config : @permissions.Config
  open : Map[@permissions.User, Set[@permissions.Doc]]
  fault : String
}

///|
impl @permissions.Permissions for Domain with fn state(self) {
  { open: self.open, }
}

///|
impl @permissions.Permissions for Domain with fn read(self, u, d) {
  let roles = self.config.grants.get(u).unwrap_or(Set([]))
  let granted = roles
    .iter()
    .any(fn(role) { self.config.reads.get(role).unwrap_or(Set([])).contains(d) })
  if !granted && self.fault != "read" {
    return false
  }
  match self.open.get(u) {
    Some(docs) => docs.insert(d)
    None => self.open[u] = Set([d])
  }
  true
}

///|
impl @permissions.Permissions for Domain with fn close(self, u, d) {
  guard self.open.get(u) is Some(docs) && docs.contains(d) else { false }
  if self.fault != "close" {
    docs.remove(d)
  }
  true
}

///|
fn main {
  let fault = @io.argument()
  let server = @permissions.Server(fn(config) {
    { config, open: Map([]), fault, }
  })
  while @io.read_line() is Some(line) {
    @io.write_line(server.handle(line))
  }
}
