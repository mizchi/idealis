# 0105: MoonBitのtraitとdecodeの生成

[0102](../iterations/0102-domain-bindings.md)・[0104](../iterations/0104-go-bindings.md)に続き、mizchiの依頼でMoonBitのbindingsを生成する。JSONの形は0102の契約と同じで、照合器も同じ。

## 生成するもの（`rmodel::bindings::moonbit(model)`）

- sigは`struct User(Int)`（JSONは`"User0"`）、enumはMoonBitのenum、和のsortは子を持つenum、Boolは`Bool`、Intは`Int64`、recordはstruct。集合は`Set[T]`、`lone`は`T?`、二項関係は`Map[A, Set[B]]`・`Map[A, B]`。
- `Config`・`State`、actionごとの構成子を持つ`Command`、モデルのtrait（`fn state(Self) -> State`と、actionごとの`fn read(Self, User, Doc) -> Bool`）。
- `decode_config`・`decode_command`は、知らないfield・欠けたfield・範囲外のatomを拒む。`encode_state`は契約のJSONを書く。
- `Server::handle(line)`が、JSON Linesのdriverの1行を受けて1行を返す。生成物はtargetに依存しない（coreにstdinがないため、入出力はテスト側のmainがJSのFFIで書く）。

## 正しさ（結果を見る前に固定）

- permissions・sessionsをMoonBitで実装し、rcheckの照合が一致すること。
- 0102と同じ人工故障3件を、長さ1・2・3の最短の列で検出すること。
- 生成物が古くないこと、`moon check --deny-warn`と`moon fmt`の差分がないこと。
- decodeの拒否: 知らないfield、欠けたfield、範囲外のatomを持つconfigとcommandを、MoonBitのテストで拒むこと。

## 範囲外

- JS以外のtargetでのdriver、mooncakesへの公開。
