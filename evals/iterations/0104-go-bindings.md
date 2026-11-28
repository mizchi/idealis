# 0104: Goのinterfaceとdecodeの生成

[plan](../plans/0104-go-bindings.md)のとおり、`idealis-check generate <model> go`でGoのpackageを生成した。JSONの形は[0102](0102-domain-bindings.md)の契約と同じで、照合器も同じ。

```go
type Permissions interface {
	State() State
	Read(u User, d Doc) bool
	Close(u User, d Doc) bool
}
func Serve(create func(Config) Permissions, in io.Reader, out io.Writer) error
```

- sigは`type User uint8`、enumは`type Currency string`の定数、Boolは`bool`、Intは`int64`、recordはstruct、和のsortは子の型が満たすinterface。集合は`Set[T]`（`map[T]struct{}`）、`lone`は`*T`、二項関係は`map[A]Set[B]`・`map[A]B`。
- `DecodeConfig`・`DecodeCommand`は、知らないfield・欠けたfield・範囲外のatom（`"User7"`、`"User01"`）を拒む。`State.Encode`は契約のJSONを書く。
- 生成物はgofmtの書式で出す（字下げを括弧の深さで付け、structとconstの列を揃える後処理）。

## 計画からの差（結果の前）

- Goのdriverは`.artifacts/idealis-check-go/`にbuildしたものを、rcheckのテストから動かす（`just idealis-check-go`、CIのGoのjobに追加）。

## 正しさ

| plan の条件                                      | 結果                                                                                            |
| ------------------------------------------------ | ----------------------------------------------------------------------------------------------- |
| permissions・sessionsをGoで実装し、照合が一致    | 一致                                                                                            |
| 0102の人工故障3件を、長さ1・2・3の最短の列で検出 | 検出                                                                                            |
| 生成物が古くない、`go vet`が通る                 | 再生成と一致。`gofmt -l`も差分なし                                                              |
| decodeの拒否                                     | 知らないfield・欠けたfield・範囲外のatom・関係の鍵の欠けを持つconfig 4件と、command 5件を拒んだ |

人工故障の検出は、この3件を検出できることを示すだけで、未知の不具合を見つける力の評価ではない。

## 範囲外のまま

- 値の集合、実装の状態の一部だけを観測すること。Goのmoduleの公開。
