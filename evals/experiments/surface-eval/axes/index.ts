// The axis registry (plan 0109). A plan names the axes it uses; adding an
// axis is adding a module here.
import type { Axis } from "../types.js";
import { cost } from "./cost.js";
import { ddd } from "./ddd.js";
import { formal } from "./formal.js";
import { human } from "./human.js";
import { machine } from "./machine.js";

export const axes: Record<string, Axis> = { ddd, formal, machine, human, cost };
