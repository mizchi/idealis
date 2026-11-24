| 入れ子（種類:大きさ） | 結果 | debug before | debug after | release before | release after |
| --- | --- | --: | --: | --: | --: |
| operator:121 | complete | 3,203 | 659 | 147 | 147 |
| operator:122 | 拒否 128 | 3,235 | 659 | 147 | 147 |
| value:60 | complete | 6,403 | 723 | 179 | 51 |
| value:61 | 拒否 128 | 6,483 | 707 | 179 | 51 |
| call:127 | complete | 419 | 419 | 35 | 35 |
| call:128 | 拒否 128 | 387 | 387 | 35 | 35 |
| membership:120 | complete | 1,171 | 1,187 | 115 | 99 |
| membership:121 | 拒否 128 | 1,043 | 1,011 | 115 | 99 |
| binding:115 | complete | 467 | 275 | 51 | 51 |
| binding:116 | 拒否 128 | 451 | 259 | 51 | 51 |
| enabled:119 | complete | 931 | 931 | 67 | 67 |
| enabled:120 | 拒否 128 | 931 | 931 | 67 | 67 |
| bind-run:115 | complete | 339 | 339 | 51 | 51 |
| bind-run:116 | 拒否 128 | 339 | 339 | 51 | 51 |
| lazy:60 | complete | 1,011 | 963 | 51 | 51 |
| lazy:61 | 拒否 128 | 1,043 | 995 | 51 | 51 |
| temporal:122 | complete | 595 | 595 | 35 | 35 |
| temporal:123 | 拒否 128 | 595 | 595 | 35 | 35 |

| 連鎖 n | debug before | debug after | release before | release after |
| --- | --: | --: | --: | --: |
| if 3 | 131 | 51 | 7 | 7 |
| if 47 | 1283 | 51 | 51 | 7 |
| if 48 | checker 48 | 51 | checker 48 | 7 |
| if 61 | checker 48 | 51 | checker 48 | 7 |
| if 127 | compile 拒否 | compile 拒否 | compile 拒否 | compile 拒否 |
| if 128 | compile 拒否 | compile 拒否 | compile 拒否 | compile 拒否 |
| flat 3 | 115 | 51 | 7 | 7 |
| flat 47 | 1251 | 51 | 51 | 7 |
| flat 48 | 1283 | 51 | 51 | 7 |
| flat 61 | checker 48 | 51 | checker 48 | 7 |
| flat 127 | checker 48 | 51 | checker 48 | 7 |
| flat 128 | checker 48 | 51 | checker 48 | 7 |
| case 3 | 131 | 51 | 7 | 7 |
| case 47 | 1283 | 51 | 51 | 7 |
| case 48 | checker 48 | 51 | checker 48 | 7 |
| case 61 | checker 48 | 51 | checker 48 | 7 |
| case 127 | checker 48 | 51 | checker 48 | 7 |
| case 128 | compile 拒否 | compile 拒否 | compile 拒否 | compile 拒否 |
