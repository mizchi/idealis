# 分割した rmodel の例

## 出金

[用語・型](withdrawal/domain.model) と [振る舞い・性質](withdrawal/withdrawal.model) を [マニフェスト](withdrawal/project.rmodel.json) で一つのモデルとして読む。

- 金額は整数単位の値。出金依頼は正の金額を受け付け、一件ずつ処理する。モデルでは `Amount` と `requestWithdrawal` が対応する。
- 依頼額が残高以下なら出金して、ちょうどその額を差し引く。残高と同額の出金もできる。`withdraw`、`SuccessDebitsAmount`、`CanWithdrawExactBalance` が対応する。
- 依頼額が残高を超えたら残高不足として処理を完了し、残高を保持する。`rejectWithdrawal`、`RejectionPreservesBalance`、`CanReject` が対応する。

## Raft の選挙

[用語・型](raft/domain.model) と [振る舞い・性質](raft/election.model) を [マニフェスト](raft/project.rmodel.json) で一つのモデルとして読む。

- 候補は自分に投票し、各サーバは同じ任期に一票を持つ。`start` と `grantVote` が対応する。
- 3ノードのうち過半数の2票で当選し、同じ任期で二人は当選しない。`elect` と `ElectionSafety` が対応する。投票メッセージの到着を `grantVote` に抽象化している。[Raft 論文 §5.2](https://files.usenix.org/s3fs-public/cdn/atc14-paper-ongaro.pdf)
- 当選と、全員が一票ずつ自分に投じて誰も当選できない票割れの両方に到達できる。`SomeLeader` と `SplitVote` が対応する。
