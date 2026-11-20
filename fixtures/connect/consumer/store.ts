import { apply, type Driver } from "./bindings";

let value = 0n;
const store = {
  actions: {
    Init() {
      value = 0n;
    },
    Put({ selected }) {
      value = selected;
    },
  },
  observe: () => ({ value }),
} satisfies Driver;

apply(store, { tag: "Init" });
apply(store, { tag: "Put", value: { selected: 9007199254740993n } });
if (store.observe().value !== 9007199254740993n) throw Error("lost precision");
