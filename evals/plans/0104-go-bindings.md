# 0104: Goのinterfaceとdecodeの生成

[0102](../iterations/0102-domain-bindings.md)のRust・TypeScriptに続き、mizchiの依頼でGoのbindingsを生成する。JSONの形は0102の契約と同じ。

## 生成するもの（`rmodel::bindings::go(model, package)`）

- sigは`type User uint8`（JSONは`"User0"`）、enumは`type Currency string`の定数、Boolは`bool`、Intは`int64`、recordはstruct、和のsortは子の型が満たすinterface。
- 集合は`Set[T]`（`map[T]struct{}`）、`lone`は`*T`、`one`は`T`。二項関係は`map[A]Set[B]`・`map[A]B`（`lone`は鍵がないとき空）。
- `Config`・`State`と、actionごとのstructを満たす`Command` interface。`DecodeConfig`・`DecodeCommand`は知らないfield・欠けたfield・範囲外のatomを拒み、`State`はJSONの形へencodeする。
- モデルのinterface（`State() State`と、actionごとの`Read(u User, d Doc) bool`）、`Serve(create, in, out)`がJSON Linesのdriverになる。

## 正しさ（結果を見る前に固定）

- permissions・sessionsをGoで実装し、rcheckの照合が一致すること。
- 0102と同じ人工故障3件を、Goでも長さ1・2・3の最短の列で検出すること。
- 生成物が古くないこと（再生成との一致）、`go vet`が通ること。
- decodeの拒否: 知らないfield、欠けたfield、範囲外のatomを持つconfigとcommandを、Goの単体テストで拒むこと。

## 範囲外

- Goのgenerics以外の集合の表し方、Goのmoduleの公開。
