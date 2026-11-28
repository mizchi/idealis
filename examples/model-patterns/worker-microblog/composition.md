# 短文投稿サービスを組み合わせる構文案

共通の型、汎用 Queue、アプリケーションの部品、検査する接続を別ソースに置く。以下は構文案。現在の検査器では [現行形式のプロジェクト](model.project.json) で同じ接続を検査できる。

## 共通の型

```ideal
// domain.ideal
export enum User = Author | Reader
export enum Post = Earlier | Later
export enum Delivery = Initial | Retry
```

`Delivery` は同じ投稿 ID の初回と再配信を区別するモデル内の識別子。この例では、列挙型を読み込むとその値もモデル内で参照できるという前提にした。

## Queue

```ideal
// queue.ideal
import { Delivery, Post } from "./domain.ideal"

export model WorkerQueue {
  port read offered: set Post
  port read ackRequests: Post -> set Delivery
  export var accepted: set Post
  export var pending: Post -> lone Delivery
  export var inFlight: Post -> lone Delivery
  export var acked: set Post

  init { no accepted, no pending, no inFlight, no acked }

  behavior enqueue(p: Post) {
    guard { p in offered, p not in accepted }
    update {
      accepted = accepted + p
      pending = pending + (p -> Initial)
    }
  }

  behavior deliver(p: Post, delivery: Delivery) {
    guard { delivery in p.pending }
    update {
      pending = pending - (p -> delivery)
      inFlight = inFlight + (p -> delivery)
    }
  }

  behavior retry(p: Post) {
    guard { Initial in p.inFlight }
    update {
      inFlight = inFlight - (p -> Initial)
      pending = pending + (p -> Retry)
    }
  }

  behavior ack(p: Post, delivery: Delivery) {
    guard { delivery in p.inFlight, (p -> delivery) in ackRequests }
    update {
      inFlight = inFlight - (p -> delivery)
      acked = acked + p
    }
  }
}
```

`WorkerQueue` が読むのは送信要求と ACK 要求。発行者の種類、投稿者、タイムラインへの反映条件はアプリケーション側が決める。実行可能な ACK は、現在配信中の組に対応する要求があるもの。

## アプリケーションの部品

```ideal
// microblog.ideal
import { Delivery, Post, User } from "./domain.ideal"

export model SocialGraph {
  export var follows: User -> set User

  init { no follows }

  export behavior follow(reader: User, author: User) {
    guard { reader = Reader, author = Author, author not in reader.follows }
    update { follows = follows + (reader -> author) }
  }

  export behavior unfollow(reader: User, author: User) {
    guard { author in reader.follows }
    update { follows = follows - (reader -> author) }
  }
}

export model PostStore {
  export var authors: Post -> lone User

  init { no authors }

  export behavior publish(p: Post, author: User) {
    guard { author = Author, no p.authors, p = Earlier or some Earlier.authors }
    update { authors = authors + (p -> author) }
  }
}

export model PublisherRelay {
  port read authors: Post -> lone User
  port read accepted: set Post
  export var offered: set Post

  init { no offered }

  behavior offer(p: Post) {
    guard {
      some p.authors,
      p not in offered,
      p = Earlier or Earlier in accepted
    }
    update { offered = offered + p }
  }
}

export model Timeline {
  port read follows: User -> set User
  port read authors: Post -> lone User
  port read inFlight: Post -> lone Delivery
  export var entries: User -> set Post
  export var applied: set Post
  export var replayed: set Post

  var appliedOnInitial: set Post
  init { no entries, no applied, no appliedOnInitial, no replayed }

  behavior apply(p: Post, author: User) {
    guard { some p.inFlight, author in p.authors }
    update {
      if { p not in applied } then {
        entries = entries + (follows.author -> p)
        applied = applied + p
        if { Initial in p.inFlight } then {
          appliedOnInitial = appliedOnInitial + p
        }
      } else {
        if { p in appliedOnInitial, Retry in p.inFlight } then {
          replayed = replayed + p
        }
      }
    }
  }

  export behavior read(reader: User, p: Post, author: User) {
    guard { p in reader.entries, author in p.authors, author in reader.follows }
  }
}

export model Consumer {
  port read inFlight: Post -> lone Delivery
  port read applied: set Post
  export var ackRequests: Post -> set Delivery

  init { no ackRequests }

  behavior requestAck(p: Post, delivery: Delivery) {
    guard {
      delivery in p.inFlight,
      p in applied,
      (p -> delivery) not in ackRequests
    }
    update { ackRequests = ackRequests + (p -> delivery) }
  }
}
```

`PostStore.authors` は投稿確定と同時に永続化した送信元。relay はこの情報から送信要求を出す。consumer は反映が済んだ投稿の配信にだけ ACK 要求を出す。

## 組み合わせと検査

```ideal
// microblog-check.ideal
import { Delivery, Post, User } from "./domain.ideal"
import { WorkerQueue } from "./queue.ideal"
import { SocialGraph, PostStore, PublisherRelay, Timeline, Consumer } from "./microblog.ideal"

export model Microblog {
  instance users = SocialGraph
  instance posts = PostStore
  instance relay = PublisherRelay
  instance queue = WorkerQueue
  instance timeline = Timeline
  instance consumer = Consumer

  connect relay.authors <- posts.authors
  connect relay.accepted <- queue.accepted
  connect queue.offered <- relay.offered
  connect queue.ackRequests <- consumer.ackRequests
  connect timeline.follows <- users.follows
  connect timeline.authors <- posts.authors
  connect timeline.inFlight <- queue.inFlight
  connect consumer.inFlight <- queue.inFlight
  connect consumer.applied <- timeline.applied

  invariant AckAfterWrite { queue.acked in timeline.applied }
  invariant IndexedPostsAreCommitted { User.(timeline.entries) in keys(posts.authors) }

  transition AckMatchesDelivery on queue.ack {
    (p -> delivery) in before(consumer.ackRequests)
  }
  transition ReplayKeepsRows on timeline.apply {
    p in before(timeline.applied) implies unchanged(timeline.entries)
  }
  transition ReadRespectsFollow on timeline.read {
    author in before(reader.(users.follows))
  }

  reachable CommittedBeforeEnqueue { some (keys(posts.authors) - queue.accepted) }
  reachable LaterAppliedFirst { Later in timeline.applied, Earlier not in timeline.applied }
  reachable RedeliveredAfterWrite { some timeline.replayed }
  reachable AckedAfterRedelivery {
    Earlier in queue.acked,
    Initial in Earlier.(consumer.ackRequests),
    Retry in Earlier.(consumer.ackRequests)
  }
  reachable DelayedInitialAckRequest {
    Retry in Earlier.(queue.inFlight),
    Initial in Earlier.(consumer.ackRequests),
    Retry not in Earlier.(consumer.ackRequests)
  }
  reachable StaleIndexRow {
    Earlier in Reader.(timeline.entries),
    Author in Earlier.(posts.authors),
    Author not in Reader.(users.follows)
  }
}
```

`connect queue.ackRequests <- consumer.ackRequests` は、Queue が consumer の要求を読む接続。要求の発行と受理には別のステップを割り当てた。その間に再配信が始まれば、古い配信への要求は残るが、新しい配信の ACK には使えない。操作を同じステップで呼び出す構文を追加する場合は、その同期の意味とこの二段階の通信を区別する。

`timeline.entries` はインスタンスが公開する関係の名前。関係の結合は `Reader.(timeline.entries)` と囲み、インスタンスのメンバー参照と読み分ける案にした。
