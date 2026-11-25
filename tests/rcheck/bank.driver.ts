import {
  Accounts,
  type Account,
  type Domain,
  serve,
} from "./generated/bank.js";

const fault = process.argv[2] === "withdraw";

function create(): Domain {
  const balance = new Map<Account, number>(Accounts.map((id) => [id, 0]));
  return {
    get state() {
      return { balance: new Map(balance) };
    },
    apply(command) {
      const current = balance.get(command.a);
      if (current === undefined || command.n <= 0) return false;
      if (command.kind === "deposit") {
        const next = current + command.n;
        if (next > 4) return false;
        balance.set(command.a, next);
        return true;
      }
      if (command.n > current) return false;
      balance.set(command.a, fault ? current : current - command.n);
      return true;
    },
  };
}

serve(create);
