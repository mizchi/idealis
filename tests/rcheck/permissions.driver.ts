// Plan 0102: a TypeScript implementation of fixtures/rmodel/permissions.model
// against the generated types, served to rcheck as a JSON Lines driver.
// argv[2] selects a fault: "read" skips the grant check, "close" leaves the
// document open.
import {
  type Config,
  type Doc,
  type Domain,
  type User,
  serve,
} from "./generated/permissions.js";

const fault = process.argv[2] ?? "none";

function create(config: Config): Domain {
  const open = new Map<User, Set<Doc>>();
  return {
    get state() {
      return { open };
    },
    apply(command) {
      switch (command.kind) {
        case "read": {
          const roles = config.grants.get(command.u) ?? new Set();
          const granted = [...roles].some((role) =>
            config.reads.get(role)?.has(command.d),
          );
          if (!granted && fault !== "read") return false;
          const docs = open.get(command.u) ?? new Set<Doc>();
          docs.add(command.d);
          open.set(command.u, docs);
          return true;
        }
        case "close": {
          const docs = open.get(command.u);
          if (docs === undefined || !docs.has(command.d)) return false;
          if (fault !== "close") docs.delete(command.d);
          return true;
        }
      }
    },
  };
}

serve(create);
