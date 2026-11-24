# 対照: 有界の結果を有界として扱う文書

このcrateのSAT backendは、steps以内のtraceだけを探す。stepsの中で反例が見つからなくても、それはsteps以内に反例がないことを示すだけで、完全な検査ではない。
