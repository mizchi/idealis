// Plan 0102: a TypeScript implementation of fixtures/rmodel/sessions.model.
// argv[2] "login" skips the capacity check.
import {
  type Config,
  type Domain,
  type User,
  serve,
} from "./generated/sessions.js";

const fault = process.argv[2] ?? "none";

function create(config: Config): Domain {
  const active = new Set<User>();
  let locked: User | undefined;
  return {
    get state() {
      return { active, locked };
    },
    apply(command) {
      const u = command.u;
      switch (command.kind) {
        case "login":
          if (active.has(u) || (active.size >= 2 && fault !== "login"))
            return false;
          active.add(u);
          return true;
        case "logout":
          return active.delete(u);
        case "lock":
          if (locked !== undefined || !config.admins.has(u) || !active.has(u))
            return false;
          locked = u;
          return true;
        case "unlock":
          if (locked !== u) return false;
          locked = undefined;
          return true;
      }
    },
  };
}

serve(create);
