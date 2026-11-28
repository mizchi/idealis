// Implements fixtures/rmodel/sessions.model (plan 0105). The argument
// "login" skips the capacity check.

///|
struct Domain {
  admins : Set[@sessions.User]
  active : Set[@sessions.User]
  mut locked : @sessions.User?
  fault : String
}

///|
impl @sessions.Sessions for Domain with fn state(self) {
  { active: Set::from_iter(self.active.iter()), locked: self.locked, }
}

///|
impl @sessions.Sessions for Domain with fn login(self, u) {
  if self.active.contains(u) ||
    (self.active.length() >= 2 && self.fault != "login") {
    return false
  }
  self.active.insert(u)
  true
}

///|
impl @sessions.Sessions for Domain with fn logout(self, u) {
  if !self.active.contains(u) {
    return false
  }
  self.active.remove(u)
  true
}

///|
impl @sessions.Sessions for Domain with fn lock(self, u) {
  if self.locked is Some(_) ||
    !self.admins.contains(u) ||
    !self.active.contains(u) {
    return false
  }
  self.locked = Some(u)
  true
}

///|
impl @sessions.Sessions for Domain with fn unlock(self, u) {
  guard self.locked is Some(held) && held == u else { false }
  self.locked = None
  true
}

///|
fn main {
  let fault = @io.argument()
  let server = @sessions.Server(fn(config) {
    { admins: config.admins, active: Set([]), locked: None, fault, }
  })
  while @io.read_line() is Some(line) {
    @io.write_line(server.handle(line))
  }
}
