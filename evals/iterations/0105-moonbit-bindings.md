# 0105: MoonBitのtraitとdecodeの生成

[plan](../plans/0105-moonbit-bindings.md)のとおり、`idealis-check generate <model> moonbit`でMoonBitのpackageを生成した。JSONの形は[0102](0102-domain-bindings.md)の契約と同じで、照合器も同じ。

```moonbit
pub(open) trait Permissions {
  fn state(Self) -> State
  fn read(Self, User, Doc) -> Bool
  fn close(Self, User, Doc) -> Bool
}
pub fn[D : Permissions] Server::handle(self : Server[D], line : String) -> String
```

- sigは`struct User(Int32)`、enumはMoonBitのenum、和のsortは子を持つenum、Boolは`Bool`、Intは`Int64`、recordはstruct。集合は`Set[T]`、`lone`は`T?`、二項関係は`Map[A, Set[B]]`・`Map[A, B]`。
- `decode_config`・`decode_command`は、知らないfield・欠けたfield・範囲外のatom（`"User7"`、`"User01"`）を`DecodeError`で拒む。
- 生成物はtargetに依存しない。coreにstdinがないので、テストのdriverはJSのFFI（Nodeの同期のfs）で1行ずつ読み書きし、`Server::handle`に渡す。

## 計画からの差（結果の前）

- 記録する生成物は、生成器の出力に`moon fmt`をかけたもの。`moon fmt`は行幅で折り返すので、生成器の出力そのものを書式に一致させなかった。`scripts/idealis-check-moonbit-fresh.sh`が、生成し直して`moon fmt`をかけたものと記録を比べ、moduleに`moon fmt`の差分がないことも確かめる。どちらのずれも検出することを、記録を崩して確かめた。
- 生成物のpackageは、使わないcodecの警告（`unused_value`）を外す。
- CIに、MoonBitを入れて`just idealis-check-moonbit`を動かすjobを足した。installerの版は固定していない。`moon fmt`の書式が版で変わると、freshnessの検査が落ちる。

## 正しさ

| plan の条件                                                        | 結果                                                                                            |
| ------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------- |
| permissions・sessionsをMoonBitで実装し、照合が一致                 | 一致（JS target）                                                                               |
| 0102の人工故障3件を、長さ1・2・3の最短の列で検出                   | 検出                                                                                            |
| 生成物が古くない、`moon check --deny-warn`と`moon fmt`の差分がない | 再生成と一致、警告なし、差分なし（moon 0.1.20261117）                                           |
| decodeの拒否                                                       | 知らないfield・欠けたfield・範囲外のatom・関係の鍵の欠けを持つconfig 4件と、command 5件を拒んだ |

人工故障の検出は、この3件を検出できることを示すだけで、未知の不具合を見つける力の評価ではない。

## 範囲外のまま

- JS以外のtargetでのdriver、mooncakesへの公開、値の集合。
