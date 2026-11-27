# rmodel の corpus の語彙と rule

<!-- Generated from fixtures/rmodel/spec.pkl by idealis-spec (plan 0092). Do not edit. -->

## もの

### プロセス（process）

ロックを取り合う実行の単位。

- id: `proc`
- モデル: `fixtures/rmodel/mutex.model` の `Proc`
- モデル: `fixtures/rmodel/declared.model` の `Proc`

### ロック（lock）

同時に 1 つのプロセスだけが持てる資源。

- id: `lock`
- モデル: `fixtures/rmodel/mutex.model` の `Lock`
- モデル: `fixtures/rmodel/declared.model` の `Lock`

### 利用者（user）

文書を開き、ログインする人。

- id: `user`
- モデル: `fixtures/rmodel/permissions.model` の `User`
- モデル: `fixtures/rmodel/permissions-open.model` の `User`
- モデル: `fixtures/rmodel/sessions.model` の `User`

### ロール（role）

利用者に付与され、読める文書を決める権限のまとまり。

- id: `role`
- モデル: `fixtures/rmodel/permissions.model` の `Role`
- モデル: `fixtures/rmodel/permissions-open.model` の `Role`

### 文書（document）

利用者が開く対象。

- id: `doc`
- モデル: `fixtures/rmodel/permissions.model` の `Doc`
- モデル: `fixtures/rmodel/permissions-open.model` の `Doc`

## 設定

### 付与（grant）

利用者にどのロールを与えたか。運用中は変わらない。

- id: `grants`
- モデル: `fixtures/rmodel/permissions.model` の `grants`
- モデル: `fixtures/rmodel/permissions-open.model` の `grants`

### 閲覧権限（read permission）

ロールが読める文書。

- id: `reads`
- モデル: `fixtures/rmodel/permissions.model` の `reads`
- モデル: `fixtures/rmodel/permissions-open.model` の `reads`

### 機密文書（secret document）

許可を受けた利用者だけが開いてよい文書。

- id: `secret`
- モデル: `fixtures/rmodel/permissions.model` の `secret`
- モデル: `fixtures/rmodel/permissions-open.model` の `secret`

### 許可を受けた利用者（cleared user）

機密文書を開いてよいと認められた利用者。

- id: `cleared`
- モデル: `fixtures/rmodel/permissions.model` の `cleared`
- モデル: `fixtures/rmodel/permissions-open.model` の `cleared`

### 管理者（administrator）

システムをロックできる利用者。

- id: `admins`
- モデル: `fixtures/rmodel/sessions.model` の `admins`

## 状態

### 保持者（holder）

いまロックを持っているプロセス。

- id: `holder`
- モデル: `fixtures/rmodel/mutex.model` の `holder`
- モデル: `fixtures/rmodel/declared.model` の `holder`

### 待機中のプロセス（waiting process）

次にロックを待っているプロセス。常にちょうど 1 つ。

- id: `waiting`
- モデル: `fixtures/rmodel/declared.model` の `waiting`

### 開いている文書（open document）

利用者がいま開いている文書。

- id: `open`
- モデル: `fixtures/rmodel/permissions.model` の `open`
- モデル: `fixtures/rmodel/permissions-open.model` の `open`

### ログイン中の利用者（active user）

いまログインしている利用者。

- id: `active`
- モデル: `fixtures/rmodel/sessions.model` の `active`

### ロックした利用者（locking user）

システムをロックしている利用者。いないこともある。

- id: `locked`
- モデル: `fixtures/rmodel/sessions.model` の `locked`

## 操作

### 取得（acquire）

空いているロックをプロセスが取る。

- id: `acquire`
- モデル: `fixtures/rmodel/mutex.model` の `acquire`

### 解放（release）

ロックを持つプロセスがそれを手放す。

- id: `release`
- モデル: `fixtures/rmodel/mutex.model` の `release`

### 奪取の試み（grab）

条件なしにロックを取ろうとする。宣言の多重度だけが、2 つ目の保持者を止める。

- id: `grab`
- モデル: `fixtures/rmodel/declared.model` の `grab`

### 待機（wait）

待機中のプロセスを入れ替える。

- id: `wait`
- モデル: `fixtures/rmodel/declared.model` の `wait`

### 閲覧（read）

付与されたロールが読める文書を、利用者が開く。

- id: `read`
- モデル: `fixtures/rmodel/permissions.model` の `read`
- モデル: `fixtures/rmodel/permissions-open.model` の `read`

### 閉じる（close）

利用者が開いている文書を閉じる。

- id: `close`
- モデル: `fixtures/rmodel/permissions.model` の `close`
- モデル: `fixtures/rmodel/permissions-open.model` の `close`

### ログイン（log in）

定員に空きがあるとき、利用者がログインする。

- id: `login`
- モデル: `fixtures/rmodel/sessions.model` の `login`

### ログアウト（log out）

ログイン中の利用者がログアウトする。

- id: `logout`
- モデル: `fixtures/rmodel/sessions.model` の `logout`

### システムのロック（lock the system）

ログイン中の管理者が、誰もロックしていないシステムをロックする。

- id: `lock-system`
- モデル: `fixtures/rmodel/sessions.model` の `lock`

### ロックの解除（unlock the system）

ロックしている利用者がロックを外す。

- id: `unlock-system`
- モデル: `fixtures/rmodel/sessions.model` の `unlock`

## rule

### MUTEX-ONE-HOLDER

1 つのロックを同時に持つプロセスは高々 1 つ。

- `fixtures/rmodel/mutex.model` の `OneHolder` は holds
- `fixtures/rmodel/declared.model` の `OneHolder` は holds

### MUTEX-USED

ロックは実際に取得され、すべてのロックが同時に持たれることもある。

- `fixtures/rmodel/mutex.model` の `NeverHeld` は violated
- `fixtures/rmodel/mutex.model` の `Handover` は reachable
- `fixtures/rmodel/declared.model` の `BothHeld` は reachable

### MUTEX-SEVERAL-LOCKS

1 つのプロセスが複数のロックを同時に持てる。

- `fixtures/rmodel/mutex.model` の `TwoLocksOneProc` は reachable

### WAIT-EXACTLY-ONE

待機中のプロセスは常にちょうど 1 つ。

- `fixtures/rmodel/declared.model` の `OneWaiting` は holds

### PERM-NO-LEAK

付与の方針の下では、機密文書を開いている利用者は、許可を受けた利用者だけ。

- `fixtures/rmodel/permissions.model` の `NoLeak` は holds
- `fixtures/rmodel/permissions.model` の `UnclearedSecret` は unreachable

### PERM-POLICY-NEEDED

付与の方針がなければ、許可を受けていない利用者が機密文書を開けてしまう。

- `fixtures/rmodel/permissions-open.model` の `NoLeak` は violated
- `fixtures/rmodel/permissions-open.model` の `UnclearedSecret` は reachable

### PERM-ONLY-GRANTED

利用者が開いている文書は、付与されたロールが読める文書だけ。

- `fixtures/rmodel/permissions.model` の `OnlyGranted` は holds
- `fixtures/rmodel/permissions-open.model` の `OnlyGranted` は holds

### PERM-SECRET-USABLE

付与の方針があっても、機密文書を開ける場合がある。

- `fixtures/rmodel/permissions.model` の `SecretRead` は reachable
- `fixtures/rmodel/permissions-open.model` の `SecretRead` は reachable

### SESS-CAPACITY

同時にログインしている利用者は 2 人まで。

- `fixtures/rmodel/sessions.model` の `Capacity` は holds
- `fixtures/rmodel/sessions.model` の `ThreeActive` は unreachable

### SESS-FULL-AND-LOCKED

定員いっぱいで、かつロックされた状態になりうる。

- `fixtures/rmodel/sessions.model` の `Full` は reachable

### SESS-LOCK-BY-ADMIN

システムをロックしているのは管理者だけ。

- `fixtures/rmodel/sessions.model` の `LockedByAdmin` は holds

### SESS-LOCK-OUTLIVES-LOGOUT

ロックした利用者がログアウトしても、ロックは残る。

- `fixtures/rmodel/sessions.model` の `LockedIsActive` は violated
