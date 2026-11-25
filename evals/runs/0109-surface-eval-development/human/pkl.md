You are reading models written in a modelling language you may not have seen. Answer every question by reasoning about the text alone; do not run tools or look anything up. Reply with one JSON object that maps each question id to exactly one of the options listed for it, and nothing else.

## Shared definitions (Model.pkl)

```
module Model

typealias Name = String(matches(Regex(#"^[A-Za-z][A-Za-z0-9_]*$"#)))
typealias Mult = "one" | "lone" | "set"
typealias Kind = "invariant" | "reachable" | "step" | "temporal"
typealias Expect = "holds" | "violated" | "reachable" | "unreachable"

abstract class Type { fixed tag: String }
class SortT extends Type { fixed tag = "sort"; name: Name }
class DataT extends Type { fixed tag = "data"; name: Name }
class IntT extends Type { fixed tag = "int" }
class BoolT extends Type { fixed tag = "bool" }

class Field { name: Name; type: Type }
class Variant { name: Name; fields: List<Field> = List() }
class Data {
  name: Name
  variants: List<Variant>(!isEmpty, map((it) -> it.name).isDistinct)
}

class Rel {
  name: Name
  mutable: Boolean
  dom: List<Type> = List()
  cod: Type
  mult: Mult
  initial: Init?(mutable == (this != null))
}
abstract class Init { fixed tag: String }
class Empty extends Init { fixed tag = "empty" }
class Const extends Init { fixed tag = "const"; value: Expr }

abstract class Expr { fixed op: String }
class Var extends Expr { fixed op = "var"; name: Name }
class IntLit extends Expr { fixed op = "int"; value: Int }
class BoolLit extends Expr { fixed op = "bool"; value: Boolean }
class Ctor extends Expr { fixed op = "ctor"; data: Name; variant: Name; args: List<Expr> = List() }
class App extends Expr { fixed op = "app"; rel: Name; args: List<Expr> = List() }
class Image extends Expr { fixed op = "image"; rel: Name; args: List<Expr> = List() }
class Universe extends Expr { fixed op = "universe"; sort: Name }
class In extends Expr { fixed op = "in"; elem: Expr; set: Expr }
class Count extends Expr { fixed op = "count"; set: Expr }
class Not extends Expr { fixed op = "not"; arg: Expr }
class And extends Expr { fixed op = "and"; args: List<Expr> }
class Or extends Expr { fixed op = "or"; args: List<Expr> }
class Implies extends Expr { fixed op = "implies"; lhs: Expr; rhs: Expr }
class Cmp extends Expr { fixed op = "cmp"; rel: "=" | "!=" | "<" | "<=" | ">" | ">="; lhs: Expr; rhs: Expr }
class Arith extends Expr { fixed op = "arith"; fn: "+" | "-"; lhs: Expr; rhs: Expr }
class All extends Expr { fixed op = "all"; var: Name; sort: Name; body: Expr }
class Exists extends Expr { fixed op = "exists"; var: Name; sort: Name; body: Expr }
class Case { variant: Name; binds: List<Name> = List(); body: Expr }
class Match extends Expr { fixed op = "match"; scrutinee: Expr; cases: List<Case> }
class Next extends Expr { fixed op = "next"; arg: Expr }
class Always extends Expr { fixed op = "always"; arg: Expr }
class Eventually extends Expr { fixed op = "eventually"; arg: Expr }

class Param { name: Name; type: Type }
abstract class Update { fixed tag: String; rel: Name }
class Assign extends Update { fixed tag = "assign"; args: List<Expr> = List(); value: Expr }
class SetLone extends Update { fixed tag = "setLone"; args: List<Expr> = List(); value: Expr? }
class Insert extends Update { fixed tag = "insert"; args: List<Expr> = List(); elem: Expr }
class Remove extends Update { fixed tag = "remove"; args: List<Expr> = List(); elem: Expr }

class Action {
  name: Name
  params: List<Param> = List()
  guard: List<Expr> = List()
  updates: List<Update>
}

class Fairness { strength: "weak" | "strong"; action: Name }
class Prop {
  name: Name
  kind: Kind
  formula: Expr
  fairness: List<Fairness>(isEmpty || kind == "temporal") = List()
  expect: Expect
  note: String? = null
}

name: Name
sorts: List<Name>
data: List<Data> = List()
rels: List<Rel>
facts: List<Expr> = List()
actions: List<Action>
props: List<Prop>
check: Check
class Check {
  sizes: Map<Name, Int>
  intMin: Int = 0
  intMax: Int = 0
  steps: Int
}

diagnostics: List<String>(isEmpty) = errors()

function sort(n: Name): Type = new SortT { name = n }
function dataT(n: Name): Type = new DataT { name = n }
function int(): Type = new IntT {}
function bool(): Type = new BoolT {}
function enumOf(n: Name, vs: List<Name>): Data = new { name = n; variants = vs.map((it) -> new Variant { name = it }) }
function param(n: Name, t: Type): Param = new { name = n; type = t }
function adt(n: Name, vs: List<Variant>): Data = new { name = n; variants = vs }
function variant(n: Name, fs: List<Field>): Variant = new { name = n; fields = fs }
function field(n: Name, t: Type): Field = new { name = n; type = t }

function v(n: Name): Expr = new Var { name = n }
function i(n: Int): Expr = new IntLit { value = n }
function lit(b: Boolean): Expr = new BoolLit { value = b }
function ctor(d: Name, vn: Name, xs: List<Expr>): Expr = new Ctor { data = d; variant = vn; args = xs }
function at(r: Name, xs: List<Expr>): Expr = new App { rel = r; args = xs }
function img(r: Name, xs: List<Expr>): Expr = new Image { rel = r; args = xs }
function univ(s: Name): Expr = new Universe { sort = s }
function mem(x: Expr, s: Expr): Expr = new In { elem = x; set = s }
function count(s: Expr): Expr = new Count { set = s }
function no(s: Expr): Expr = cmp("=", count(s), i(0))
function not(x: Expr): Expr = new Not { arg = x }
function and(xs: List<Expr>): Expr = new And { args = xs }
function or(xs: List<Expr>): Expr = new Or { args = xs }
function implies(a: Expr, b: Expr): Expr = new Implies { lhs = a; rhs = b }
function cmp(r: String, a: Expr, b: Expr): Expr = new Cmp { rel = r; lhs = a; rhs = b }
function eq(a: Expr, b: Expr): Expr = cmp("=", a, b)
function plus(a: Expr, b: Expr): Expr = new Arith { fn = "+"; lhs = a; rhs = b }
function minus(a: Expr, b: Expr): Expr = new Arith { fn = "-"; lhs = a; rhs = b }
function all(x: Name, s: Name, fn: (Expr) -> Expr): Expr = new All { var = x; sort = s; body = fn.apply(v(x)) }
function some(x: Name, s: Name, fn: (Expr) -> Expr): Expr = new Exists { var = x; sort = s; body = fn.apply(v(x)) }
function match(e: Expr, cs: List<Case>): Expr = new Match { scrutinee = e; cases = cs }
function alt(vn: Name, bs: List<Name>, b: Expr): Case = new { variant = vn; binds = bs; body = b }
function next(e: Expr): Expr = new Next { arg = e }
function always(e: Expr): Expr = new Always { arg = e }
function eventually(e: Expr): Expr = new Eventually { arg = e }

function config(n: Name, d: List<Type>, m: Mult, c: Type): Rel =
  new { name = n; mutable = false; dom = d; mult = m; cod = c; initial = null }
function state(n: Name, d: List<Type>, m: Mult, c: Type, ini: Init): Rel =
  new { name = n; mutable = true; dom = d; mult = m; cod = c; initial = ini }
function empty(): Init = new Empty {}
function constInit(e: Expr): Init = new Const { value = e }
function action(n: Name, ps: List<Param>, g: List<Expr>, us: List<Update>): Action =
  new { name = n; params = ps; guard = g; updates = us }
function assign(r: Name, xs: List<Expr>, e: Expr): Update = new Assign { rel = r; args = xs; value = e }
function setLone(r: Name, xs: List<Expr>, e: Expr?): Update = new SetLone { rel = r; args = xs; value = e }
function insert(r: Name, xs: List<Expr>, e: Expr): Update = new Insert { rel = r; args = xs; elem = e }
function remove(r: Name, xs: List<Expr>, e: Expr): Update = new Remove { rel = r; args = xs; elem = e }
function prop(n: Name, k: Kind, x: Expect, f: Expr): Prop = new { name = n; kind = k; expect = x; formula = f }
function fairProp(n: Name, x: Expect, fs: List<Fairness>, f: Expr): Prop =
  new { name = n; kind = "temporal"; expect = x; fairness = fs; formula = f }
function weak(a: Name): Fairness = new { strength = "weak"; action = a }
function scope(m: Map<Name, Int>, lo: Int, hi: Int, k: Int): Check = new { sizes = m; intMin = lo; intMax = hi; steps = k }

function tname(t: Type): String =
  if (t is SortT) t.name else if (t is DataT) t.name else if (t is IntT) "Int" else "Bool"

function relOf(n: String): Rel? = rels.findOrNull((r) -> r.name == n)
function dataOf(n: String): Data? = data.findOrNull((d) -> d.name == n)
function domainOk(n: String): Boolean = sorts.contains(n) || dataOf(n) != null

function typeOf(e: Expr, env: Map<String, String>): String =
  if (e is Var) env.getOrNull(e.name) ?? "?"
  else if (e is IntLit || e is Count || e is Arith) "Int"
  else if (e is Ctor) e.data
  else if (e is App) let (r = relOf(e.rel)) if (r == null) "?" else tname(r.cod)
  else if (e is Image) let (r = relOf(e.rel)) if (r == null) "?" else "{" + tname(r.cod) + "}"
  else if (e is Universe) "{" + e.sort + "}"
  else if (e is Next) typeOf(e.arg, env)
  else if (e is Match)
    let (d = dataOf(typeOf(e.scrutinee, env)))
      if (d == null || e.cases.isEmpty) "?"
      else typeOf(e.cases.first.body, caseEnv(d, e.cases.first, env))
  else "Bool"

function caseEnv(d: Data, c: Case, env: Map<String, String>): Map<String, String> =
  let (vr = d.variants.findOrNull((it) -> it.name == c.variant))
    if (vr == null || vr.fields.length != c.binds.length) env
    else c.binds.zip(vr.fields).fold(env, (acc, p) -> acc.put(p.first, tname(p.second.type)))

function expect(e: Expr, t: String, env: Map<String, String>, mode: String, where: String): List<String> =
  let (es = check(e, env, mode, where))
    if (!es.isEmpty) es
    else let (got = typeOf(e, env)) if (got == t) List() else List("\(where): expected \(t), got \(got)")

function args(r: Rel, xs: List<Expr>, env: Map<String, String>, mode: String, where: String): List<String> =
  if (r.dom.length != xs.length) List("\(where): \(r.name) takes \(r.dom.length) arguments, got \(xs.length)")
  else xs.zip(r.dom).flatMap((p) -> expect(p.first, tname(p.second), env, mode, where))

function relUse(n: String, mode: String, where: String): List<String> =
  let (r = relOf(n))
    if (r == null) List("\(where): undeclared relation \(n)")
    else if (mode == "fact" && r.mutable) List("\(where): a fact may only use configuration, not \(n)")
    else List()

function check(e: Expr, env: Map<String, String>, mode: String, where: String): List<String> =
  if (e is Var) (if (env.containsKey(e.name)) List() else List("\(where): unbound variable \(e.name)"))
  else if (e is IntLit || e is BoolLit) List()
  else if (e is Universe) (if (domainOk(e.sort)) List() else List("\(where): undeclared sort \(e.sort)"))
  else if (e is Ctor)
    let (d = dataOf(e.data))
    let (vr = d?.variants?.findOrNull((it) -> it.name == e.variant))
      if (d == null) List("\(where): undeclared data type \(e.data)")
      else if (vr == null) List("\(where): \(e.data) has no variant \(e.variant)")
      else if (vr.fields.length != e.args.length) List("\(where): \(e.variant) takes \(vr.fields.length) fields")
      else e.args.zip(vr.fields).flatMap((p) -> expect(p.first, tname(p.second.type), env, mode, where))
  else if (e is App || e is Image)
    let (u = relUse(e.rel, mode, where))
      if (!u.isEmpty) u
      else let (r = relOf(e.rel)!!)
        if (e is App && r.mult != "one") List("\(where): \(r.name) is \(r.mult); use its image")
        else if (e is Image && r.mult == "one") List("\(where): \(r.name) is one; use its value")
        else args(r, e.args, env, mode, where)
  else if (e is In)
    let (es = check(e.elem, env, mode, where) + check(e.set, env, mode, where))
      if (!es.isEmpty) es
      else let (te = typeOf(e.elem, env)) let (ts = typeOf(e.set, env))
        if (ts == "{" + te + "}") List() else List("\(where): \(te) is not an element of \(ts)")
  else if (e is Count)
    let (es = check(e.set, env, mode, where))
      if (!es.isEmpty) es else if (typeOf(e.set, env).startsWith("{")) List() else List("\(where): # of a non-set")
  else if (e is Not) expect(e.arg, "Bool", env, mode, where)
  else if (e is And) e.args.flatMap((x) -> expect(x, "Bool", env, mode, where))
  else if (e is Or) e.args.flatMap((x) -> expect(x, "Bool", env, mode, where))
  else if (e is Implies) expect(e.lhs, "Bool", env, mode, where) + expect(e.rhs, "Bool", env, mode, where)
  else if (e is Arith) expect(e.lhs, "Int", env, mode, where) + expect(e.rhs, "Int", env, mode, where)
  else if (e is Cmp)
    if (e.rel == "=" || e.rel == "!=")
      let (es = check(e.lhs, env, mode, where) + check(e.rhs, env, mode, where))
        if (!es.isEmpty) es
        else let (a = typeOf(e.lhs, env)) let (b = typeOf(e.rhs, env))
          if (a == b) List() else List("\(where): comparing \(a) with \(b)")
    else expect(e.lhs, "Int", env, mode, where) + expect(e.rhs, "Int", env, mode, where)
  else if (e is All || e is Exists)
    if (!domainOk(e.sort)) List("\(where): undeclared sort \(e.sort)")
    else expect(e.body, "Bool", env.put(e.var, e.sort), mode, where)
  else if (e is Match)
    let (es = check(e.scrutinee, env, mode, where))
      if (!es.isEmpty) es
      else let (d = dataOf(typeOf(e.scrutinee, env)))
        if (d == null) List("\(where): match on a value of \(typeOf(e.scrutinee, env)), not a data type")
        else let (names = e.cases.map((c) -> c.variant))
          let (missing = d.variants.map((it) -> it.name).filter((n) -> !names.contains(n)))
          if (!names.isDistinct) List("\(where): duplicate case in match on \(d.name)")
          else if (!missing.isEmpty) List("\(where): match on \(d.name) misses \(missing.join(", "))")
          else let (t = typeOf(e, env))
            e.cases.flatMap((c) ->
              let (vr = d.variants.find((it) -> it.name == c.variant))
                if (vr.fields.length != c.binds.length) List("\(where): \(c.variant) binds \(vr.fields.length) fields")
                else expect(c.body, t, caseEnv(d, c, env), mode, where))
  else if (e is Next)
    if (mode == "step" || mode == "temporal") check(e.arg, env, "state", where)
    else List("\(where): next-state values only in step or temporal properties")
  else if (e is Always || e is Eventually)
    if (mode == "temporal") expect(e.arg, "Bool", env, mode, where)
    else List("\(where): temporal operators only in temporal properties")
  else List("\(where): unknown node")

function typeErrors(t: Type, where: String): List<String> =
  if (t is SortT && !sorts.contains(t.name)) List("\(where): undeclared sort \(t.name)")
  else if (t is DataT && dataOf(t.name) == null) List("\(where): undeclared data type \(t.name)")
  else List()

function updateErrors(a: Action, u: Update, env: Map<String, String>): List<String> =
  let (where = "action \(a.name), update \(u.rel)")
  let (r = relOf(u.rel))
    if (r == null) List("\(where): undeclared relation")
    else if (!r.mutable) List("\(where): \(u.rel) is configuration and cannot change")
    else if (u is Assign && r.mult != "one") List("\(where): assign needs a one relation")
    else if (u is SetLone && r.mult != "lone") List("\(where): setLone needs a lone relation")
    else if ((u is Insert || u is Remove) && r.mult != "set") List("\(where): insert/remove needs a set relation")
    else args(r, u.args, env, "state", where) +
      (if (u is Assign) expect(u.value, tname(r.cod), env, "state", where)
       else if (u is SetLone) (if (u.value == null) List() else expect(u.value!!, tname(r.cod), env, "state", where))
       else if (u is Insert) expect(u.elem, tname(r.cod), env, "state", where)
       else if (u is Remove) expect(u.elem, tname(r.cod), env, "state", where)
       else List())

function errors(): List<String> =
  (if (sorts.isDistinct) List() else List("sorts are not distinct")) +
  (if ((sorts + data.map((d) -> d.name)).isDistinct) List() else List("a data type shares a sort's name")) +
  (if (rels.map((r) -> r.name).isDistinct) List() else List("relations are not distinct")) +
  data.flatMap((d) -> d.variants.flatMap((vr) -> vr.fields.flatMap((f) -> typeErrors(f.type, "data \(d.name)")))) +
  rels.flatMap((r) ->
    r.dom.flatMap((t) -> typeErrors(t, "relation \(r.name)")) + typeErrors(r.cod, "relation \(r.name)") +
    (if (r.initial is Const && r.mult != "one") List("relation \(r.name): const initial needs a one relation")
     else if (r.initial is Empty && r.mult == "one") List("relation \(r.name): a one relation cannot start empty")
     else if (r.initial is Const) expect((r.initial as Const).value, tname(r.cod), Map(), "state", "relation \(r.name)")
     else List())) +
  facts.flatMap((f) -> expect(f, "Bool", Map(), "fact", "fact")) +
  actions.flatMap((a) ->
    let (env = a.params.fold(Map(), (acc, p) -> acc.put(p.name, tname(p.type))))
      a.params.flatMap((p) -> typeErrors(p.type, "action \(a.name)")) +
      a.guard.flatMap((g) -> expect(g, "Bool", env, "state", "action \(a.name)")) +
      a.updates.flatMap((u) -> updateErrors(a, u, env))) +
  props.flatMap((p) ->
    let (mode = if (p.kind == "step") "step" else if (p.kind == "temporal") "temporal" else "state")
      expect(p.formula, "Bool", Map(), mode, "property \(p.name)") +
      p.fairness.flatMap((f) -> if (actions.any((a) -> a.name == f.action)) List() else List("property \(p.name): fairness on undeclared action \(f.action)")) +
      (if ((p.kind == "invariant" || p.kind == "step" || p.kind == "temporal") && !(p.expect == "holds" || p.expect == "violated"))
         List("property \(p.name): \(p.kind) expects holds or violated")
       else if (p.kind == "reachable" && !(p.expect == "reachable" || p.expect == "unreachable"))
         List("property \(p.name): reachable expects reachable or unreachable")
       else List())) +
  check.sizes.keys.toList().flatMap((s) -> if (sorts.contains(s)) List() else List("check: size for undeclared sort \(s)")) +
  sorts.flatMap((s) -> if (check.sizes.containsKey(s)) List() else List("check: no size for sort \(s)"))

```

## Model permissions

```
amends "Model.pkl"

name = "Permissions"
sorts = List("User", "Role", "Doc")
rels = List(
  config("grants", List(sort("User")), "set", sort("Role")),
  config("reads", List(sort("Role")), "set", sort("Doc")),
  config("secret", List(), "set", sort("Doc")),
  config("cleared", List(), "set", sort("User")),
  state("open", List(sort("User")), "set", sort("Doc"), empty()),
)
local secret = img("secret", List())
local cleared = img("cleared", List())
local function grants(u: Expr): Expr = img("grants", List(u))
local function reads(r: Expr): Expr = img("reads", List(r))
local function opened(u: Expr): Expr = img("open", List(u))
local function canRead(u: Expr, d: Expr): Expr =
  some("r", "Role", (r) -> and(List(mem(r, grants(u)), mem(d, reads(r)))))
local function secretOpen(u: Expr): Expr =
  some("d", "Doc", (d) -> and(List(mem(d, opened(u)), mem(d, secret))))

facts = List(
  all("r", "Role", (r) -> all("d", "Doc", (d) -> all("u", "User", (u) ->
    implies(and(List(mem(d, reads(r)), mem(d, secret), mem(r, grants(u)))), mem(u, cleared))))),
)
actions = List(
  action("read", List(param("u", sort("User")), param("d", sort("Doc"))),
    List(canRead(v("u"), v("d"))),
    List(insert("open", List(v("u")), v("d")))),
  action("close", List(param("u", sort("User")), param("d", sort("Doc"))),
    List(mem(v("d"), opened(v("u")))),
    List(remove("open", List(v("u")), v("d")))),
)
props = List(
  prop("NoLeak", "invariant", "holds",
    all("u", "User", (u) -> implies(secretOpen(u), mem(u, cleared)))),
  prop("OnlyGranted", "invariant", "holds",
    all("u", "User", (u) -> all("d", "Doc", (d) -> implies(mem(d, opened(u)), canRead(u, d))))),
  prop("SecretRead", "reachable", "reachable",
    some("u", "User", (u) -> secretOpen(u))),
  prop("UnclearedSecret", "reachable", "unreachable",
    some("u", "User", (u) -> and(List(not(mem(u, cleared)), secretOpen(u))))),
)
check = scope(Map("User", 2, "Role", 2, "Doc", 2), 0, 0, 6)

```

- `permissions.noRole`: Can a user end up with a document open that none of the user's roles can read? Options: "yes", "no"
- `permissions.close`: After a user closes a document the user had open, is that document still open for that user? Options: "yes", "no"
- `permissions.uncleared`: Can a user who is not cleared ever have a secret document open? Options: "yes", "no"
- `permissions.grants`: Can any action change which roles a user is granted? Options: "yes", "no"

## Model bank

```
amends "Model.pkl"

name = "Bank"
sorts = List("Account")
rels = List(state("balance", List(sort("Account")), "one", int(), constInit(i(0))))
local function bal(a: Expr): Expr = at("balance", List(a))
local accountAmount = List(param("a", sort("Account")), param("n", int()))
actions = List(
  action("deposit", accountAmount,
    List(cmp(">", v("n"), i(0)), cmp("<=", plus(bal(v("a")), v("n")), i(4))),
    List(assign("balance", List(v("a")), plus(bal(v("a")), v("n"))))),
  action("withdraw", accountAmount,
    List(cmp(">", v("n"), i(0)), cmp(">=", bal(v("a")), v("n"))),
    List(assign("balance", List(v("a")), minus(bal(v("a")), v("n"))))),
)
props = List(
  prop("NonNegative", "invariant", "holds", all("a", "Account", (a) -> cmp(">=", bal(a), i(0)))),
  prop("Rich", "reachable", "reachable", some("a", "Account", (a) -> cmp(">=", bal(a), i(3)))),
)
check = scope(Map("Account", 2), -4, 4, 4)

```

- `bank.cap`: Can a deposit take an account's balance above 4? Options: "yes", "no"
- `bank.overdraw`: With a balance of 2, can a withdrawal of 3 be applied? Options: "yes", "no"
- `bank.steps`: Starting from the initial state, what is the fewest number of actions after which some account has a balance of at least 3? Options: "0", "1", "2", "3", "never"

## Model order

```
amends "Model.pkl"

name = "Order"
sorts = List("Order")
data = List(
  enumOf("Carrier", List("Post", "Courier")),
  enumOf("Reason", List("Customer", "Fraud")),
  adt("Status", List(
    variant("Draft", List()),
    variant("Placed", List()),
    variant("Shipped", List(field("tracking", dataT("Carrier")))),
    variant("Cancelled", List(field("reason", dataT("Reason")))),
  )),
)
local function st(o: Expr): Expr = at("status", List(o))
local function status(vn: String, xs: List<Expr>): Expr = ctor("Status", vn, xs)
rels = List(state("status", List(sort("Order")), "one", dataT("Status"), constInit(status("Draft", List()))))
actions = List(
  action("place", List(param("o", sort("Order"))),
    List(eq(st(v("o")), status("Draft", List()))),
    List(assign("status", List(v("o")), status("Placed", List())))),
  action("ship", List(param("o", sort("Order")), param("c", dataT("Carrier"))),
    List(eq(st(v("o")), status("Placed", List()))),
    List(assign("status", List(v("o")), status("Shipped", List(v("c")))))),
  action("cancel", List(param("o", sort("Order")), param("r", dataT("Reason"))),
    List(match(st(v("o")), List(
      alt("Draft", List(), lit(true)),
      alt("Placed", List(), lit(true)),
      alt("Shipped", List("t"), lit(false)),
      alt("Cancelled", List("x"), lit(false))))),
    List(assign("status", List(v("o")), status("Cancelled", List(v("r")))))),
)
props = List(
  prop("ShippedStays", "step", "holds", all("o", "Order", (o) -> match(st(o), List(
    alt("Draft", List(), lit(true)),
    alt("Placed", List(), lit(true)),
    alt("Shipped", List("t"), eq(next(st(o)), status("Shipped", List(v("t"))))),
    alt("Cancelled", List("x"), lit(true)))))),
  prop("FraudCancelled", "reachable", "reachable", some("o", "Order", (o) -> match(st(o), List(
    alt("Draft", List(), lit(false)),
    alt("Placed", List(), lit(false)),
    alt("Shipped", List("t"), lit(false)),
    alt("Cancelled", List("r"), eq(v("r"), ctor("Reason", "Fraud", List()))))))),
  (prop("ShippedCancelled", "reachable", "unreachable", some("o", "Order", (o) -> and(List(
    match(st(o), List(alt("Draft", List(), lit(false)), alt("Placed", List(), lit(false)),
      alt("Shipped", List("t"), lit(true)), alt("Cancelled", List("r"), lit(false)))),
    match(st(o), List(alt("Draft", List(), lit(false)), alt("Placed", List(), lit(false)),
      alt("Shipped", List("t"), lit(false)), alt("Cancelled", List("r"), lit(true))))))))) {
    note = "unrepresentable state: the type of status rules it out"
  },
)
check = scope(Map("Order", 2), 0, 0, 4)

```

- `order.shippedCancel`: Can an order that has been shipped later become cancelled? Options: "yes", "no"
- `order.draftShip`: Can an order go directly from draft to shipped in one action? Options: "yes", "no"
- `order.tracking`: After an order is shipped with the carrier Post, which carrier does its state record? Options: "Post", "Courier", "none"
- `order.fraud`: Can an order that was placed be cancelled with the reason Fraud? Options: "yes", "no"

## Model sessions

```
amends "Model.pkl"

name = "Sessions"
sorts = List("User")
rels = List(
  config("admins", List(), "set", sort("User")),
  state("active", List(), "set", sort("User"), empty()),
  state("locked", List(), "lone", sort("User"), empty()),
)
local admins = img("admins", List())
local active = img("active", List())
local locked = img("locked", List())
local user = List(param("u", sort("User")))
facts = List(cmp(">", count(admins), i(0)))
actions = List(
  action("login", user, List(not(mem(v("u"), active)), cmp("<", count(active), i(2))),
    List(insert("active", List(), v("u")))),
  action("logout", user, List(mem(v("u"), active)),
    List(remove("active", List(), v("u")))),
  action("lock", user, List(no(locked), mem(v("u"), admins), mem(v("u"), active)),
    List(setLone("locked", List(), v("u")))),
  action("unlock", user, List(mem(v("u"), locked)),
    List(setLone("locked", List(), null))),
)
props = List(
  prop("Capacity", "invariant", "holds", cmp("<=", count(active), i(2))),
  prop("LockedByAdmin", "invariant", "holds", all("u", "User", (u) -> implies(mem(u, locked), mem(u, admins)))),
  prop("LockedIsActive", "invariant", "violated", all("u", "User", (u) -> implies(mem(u, locked), mem(u, active)))),
  prop("Full", "reachable", "reachable", and(List(eq(count(active), i(2)), cmp(">", count(locked), i(0))))),
  prop("ThreeActive", "reachable", "unreachable", eq(count(active), i(3))),
)
check = scope(Map("User", 3), 0, 0, 8)

```

- `sessions.max`: What is the largest number of users that can be active at the same time? Options: "1", "2", "3"
- `sessions.lockedInactive`: Can there be a locked user who is not active? Options: "yes", "no"
- `sessions.nonAdmin`: Can a user who is not an admin become the locked user? Options: "yes", "no"
- `sessions.twoLocked`: Can two different users be locked at the same time? Options: "yes", "no"

## Model service

```
amends "Model.pkl"

name = "Service"
sorts = List("Client")
rels = List(
  state("waiting", List(), "set", sort("Client"), empty()),
  state("served", List(), "set", sort("Client"), empty()),
)
local waiting = img("waiting", List())
local served = img("served", List())
local client = List(param("c", sort("Client")))
actions = List(
  action("request", client, List(not(mem(v("c"), waiting)), not(mem(v("c"), served))),
    List(insert("waiting", List(), v("c")))),
  action("serve", client, List(mem(v("c"), waiting)),
    List(remove("waiting", List(), v("c")), insert("served", List(), v("c")))),
)
local eventuallyServed = all("c", "Client", (c) -> always(implies(mem(c, waiting), eventually(mem(c, served)))))
props = List(
  fairProp("EventuallyServed", "holds", List(weak("serve")), eventuallyServed),
  fairProp("EventuallyServedUnfair", "violated", List(), eventuallyServed),
  prop("Disjoint", "invariant", "holds", all("c", "Client", (c) -> not(and(List(mem(c, waiting), mem(c, served)))))),
)
check = scope(Map("Client", 2), 0, 0, 4)

```

- `service.both`: Can a client be waiting and served at the same time? Options: "yes", "no"
- `service.unfair`: If nothing forces the serve action to happen, must every waiting client eventually be served? Options: "yes", "no"
- `service.again`: Can a client that has been served make a new request? Options: "yes", "no"
