import { bind } from "./callbacks";

const state = new Map<string, bigint>();
const driver = bind({
  onInit: () => state.clear(),
  onPut: ({ selected }) => { state.set("v", selected); },
  read: () => ({ value: state.get("v") ?? 0n }),
});
driver.actions.Put({ selected: 17n });
if (state.get("v") !== 17n) throw Error("custom adapter did not call the implementation");
