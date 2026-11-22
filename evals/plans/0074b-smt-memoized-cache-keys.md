# 0074への追記: script cache keyの初回計算と保持

[0074aの比較](../runs/0074a-smt-comparison/result.json)はsolver benchの全18ケースで閾値を満たし`adopt`になった。一方、[cache benchの比較](../runs/0074a-smt-cache-comparison/result.json)では、warmのscriptがcacheなしより遅くなった。`script-cache-warm-scopes-32`は4.708/4.816倍、`script-cache-warm-ite-64`は4.583/4.761倍。0074では1回目の`script-cache-warm`が0.375〜0.527倍だった。cache benchは0074で合否にしていなかったが、有効にしたcacheが反復で遅くなるのは機能の退行として扱う。

原因は、0074aでkeyを`solve_incremental`の各呼び出しで計算するようにしたこと。変更は、queryごとにkeyを最初に必要になったときだけ計算し、`OnceLock`で保持すること。`Script::parse`と`solve`はkeyを計算しない。keyの値、cacheの意味、APIは変えない。

測定は0074aと同じ手順で1組だけ行う。保存済みのbaseline bench binaryを再利用し、新しいcandidateのsolver/cache benchをビルドする。baseline→candidate、candidate→baseline、cache benchの開発・確認を`0074b-*`へ保存する。

- solver bench: 0074のplanと同じ閾値。
- cache bench（この追記で合否に加える）: 3 fixtureの`script-cache-warm`が両runで対応する`script-prepared`の1.0倍以下。`cache-miss-N`は引き続き報告だけにする。

超えたケースは後退として記録し、さらに再測定しない。0074と0074aのrunは上書きしない。正しさのrun（seed 0x74a1/0x74c3）はこの変更後のcandidateで実行する。
