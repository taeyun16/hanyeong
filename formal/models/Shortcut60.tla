----------------------------- MODULE Shortcut60 -----------------------------
(***************************************************************************)
(* 시스템 단축키 60번(`이전 입력 소스 선택`)을 hanyeong 이 자기 F-키로 바꿨다가  *)
(* 되돌리는 순서. 강제 종료, 다시 띄우기, `restore`, 그리고 남(사용자나 다른      *)
(* 도구)이 그 사이에 60번을 바꾸는 일을 모든 순서로 섞어 본다.                   *)
(*                                                                         *)
(* 코드와의 대응은 formal/coverage.json 에 있다. 동작 하나가 설정 쓰기 하나다.    *)
(***************************************************************************)

CONSTANTS
    Targets,        \* hanyeong 을 띄울 때 고를 수 있는 F-키 (--target)
    Others,         \* 남이 60번에 쓸 수 있는 값. hanyeong 의 F-키와 겹치지 않는다
    Design,         \* "record": 지금 코드. 써 넣은 키와 원래 값을 한 값으로 남긴다.
                    \* "flag": 2026-11-17 에 고치기 전의 코드. 표시와 원래 값을 따로 남겼다
    OthersMidCall   \* TRUE 면 claim·release 한 번이 도는 사이에도 남이 60번을 쓴다

ASSUME Design \in {"flag", "record"}
ASSUME OthersMidCall \in BOOLEAN
ASSUME Targets \cap Others = {}

Absent == "absent"  \* 60번 항목이 없다(macOS 기본값이 적용된다)
Nobody == "-"       \* 도는 프로세스가 없을 때의 target 과 seen
Foreign == Others \cup {Absent}
Values == Targets \cup Foreign

VARIABLES
    sys,        \* com.apple.symbolichotkeys 의 60번 값
    saved,      \* dev.hanyeong 에 남긴 표시: 60번을 바꿔 두었다
    original,   \* dev.hanyeong 에 남긴 원래 값
    written,    \* (record 설계만) 우리가 60번에 써 넣었을 수 있는 F-키들
    pc,         \* hanyeong 프로세스가 어디까지 했는가. "off" 는 프로세스가 없다
    target,     \* 이번 실행의 --target
    seen,       \* 이번 claim·release 가 읽어 둔 60번 값
    truth,      \* 유령 변수: 남이 마지막으로 정한 값. hanyeong 이 없었다면 60번에 있을 값이다
    byUs,       \* 유령 변수: 지금 60번의 값을 hanyeong 의 claim 이 써 넣었다
    whole       \* 유령 변수: 표시를 세운 뒤 원래 값도 남겼다

vars == <<sys, saved, original, written, pc, target, seen, truth, byUs, whole>>
stored == <<saved, original, written, whole>>
ghost == <<truth, byUs>>

Init ==
    /\ sys \in Foreign
    /\ truth = sys
    /\ saved = FALSE
    /\ original = Absent
    /\ written = {}
    /\ pc = "off"
    /\ target = Nobody
    /\ seen = Nobody
    /\ byUs = FALSE
    /\ whole = TRUE

\* 지금 60번의 값 v 가 우리가 써 넣은 것인가. 기록으로 판단한다. flag 설계는 이번 실행의
\* --target 과 견주었다.
Ours(v) == IF Design = "record" THEN saved /\ v \in written ELSE v = target

Leave == pc' = "off" /\ target' = Nobody /\ seen' = Nobody

(* `hanyeong run`: app::run 이 hotkey::claim 을 부른다 *)
Start(t) ==
    /\ pc = "off"
    /\ pc' = "claim" /\ target' = t
    /\ UNCHANGED <<sys, stored, seen, ghost>>

(* `hanyeong restore`: app::restore 가 hotkey::release 를 부른다 *)
StartRestore(t) ==
    /\ pc = "off"
    /\ pc' = "release" /\ target' = t
    /\ UNCHANGED <<sys, stored, seen, ghost>>

(* claim: 60번을 읽는다. 이미 이번 F-키면 아무것도 쓰지 않는다 *)
ClaimRead ==
    /\ pc = "claim"
    /\ seen' = sys
    /\ pc' = IF sys = target THEN "on" ELSE "keep"
    /\ UNCHANGED <<sys, stored, target, ghost>>

(* claim, flag 설계: 표시가 없을 때만 남긴다. 표시를 먼저 쓴다 *)
KeepFlag ==
    /\ pc = "keep" /\ Design = "flag"
    /\ IF saved
       THEN pc' = "write" /\ UNCHANGED <<saved, whole>>
       ELSE pc' = "keep2" /\ saved' = TRUE /\ whole' = FALSE
    /\ UNCHANGED <<sys, original, written, target, seen, ghost>>

(* claim, flag 설계: 표시 다음에 원래 값을 쓴다 *)
KeepValue ==
    /\ pc = "keep2"
    /\ original' = seen /\ whole' = TRUE
    /\ pc' = "write"
    /\ UNCHANGED <<sys, saved, written, target, seen, ghost>>

(* claim, record 설계: 한 번의 쓰기. 60번이 아직 우리 값이면 원래 값은 두고 키만 더한다 *)
KeepRecord ==
    /\ pc = "keep" /\ Design = "record"
    /\ IF saved /\ seen \in written
       THEN written' = written \cup {target} /\ UNCHANGED <<saved, original>>
       ELSE saved' = TRUE /\ original' = seen /\ written' = {target}
    /\ pc' = "write"
    /\ UNCHANGED <<sys, whole, target, seen, ghost>>

(* claim: 60번을 이번 F-키로 쓴다 *)
ClaimWrite ==
    /\ pc = "write"
    /\ sys' = target /\ byUs' = TRUE
    /\ pc' = "on"
    /\ UNCHANGED <<stored, target, seen, truth>>

(* 메뉴의 종료, SIGINT·SIGTERM·SIGHUP: app::undo 가 hotkey::release 를 부른다 *)
Quit ==
    /\ pc = "on"
    /\ pc' = "release"
    /\ UNCHANGED <<sys, stored, target, seen, ghost>>

(* release: 표시가 없으면 손대지 않는다. 있으면 60번을 읽어 우리 값인지 본다 *)
ReleaseRead ==
    /\ pc = "release"
    /\ IF saved
       THEN /\ seen' = sys
            /\ pc' = IF Ours(sys) THEN "restore" ELSE "forget"
            /\ UNCHANGED target
       ELSE Leave
    /\ UNCHANGED <<sys, stored, ghost>>

(* release: 남겨 둔 원래 값을 60번에 쓴다 *)
Restore ==
    /\ pc = "restore"
    /\ sys' = original /\ byUs' = FALSE
    /\ pc' = "forget"
    /\ UNCHANGED <<stored, target, seen, truth>>

(* release, flag 설계: 표시를 먼저 지운다 *)
ForgetFlag ==
    /\ pc = "forget" /\ Design = "flag"
    /\ saved' = FALSE /\ whole' = TRUE
    /\ pc' = "forget2"
    /\ UNCHANGED <<sys, original, written, target, seen, ghost>>

(* release, flag 설계: 표시 다음에 원래 값을 지운다 *)
ForgetValue ==
    /\ pc = "forget2"
    /\ original' = Absent
    /\ Leave
    /\ UNCHANGED <<sys, saved, written, whole, ghost>>

(* release, record 설계: 한 번의 쓰기로 지운다 *)
ForgetRecord ==
    /\ pc = "forget" /\ Design = "record"
    /\ saved' = FALSE /\ original' = Absent /\ written' = {}
    /\ Leave
    /\ UNCHANGED <<sys, whole, ghost>>

(* SIGKILL, 전원 끊김, 패닉: 어느 쓰기 사이에서든 프로세스가 사라진다 *)
Crash ==
    /\ pc # "off"
    /\ Leave
    /\ UNCHANGED <<sys, stored, ghost>>

(* 사용자가 시스템 설정에서, 또는 다른 도구가 60번을 바꾼다 *)
OtherWrites(v) ==
    /\ OthersMidCall \/ pc \in {"off", "on"}
    /\ sys # v
    /\ sys' = v /\ truth' = v /\ byUs' = FALSE
    /\ UNCHANGED <<stored, pc, target, seen>>

Next ==
    \/ \E t \in Targets : Start(t) \/ StartRestore(t)
    \/ ClaimRead \/ KeepFlag \/ KeepValue \/ KeepRecord \/ ClaimWrite
    \/ Quit
    \/ ReleaseRead \/ Restore \/ ForgetFlag \/ ForgetValue \/ ForgetRecord
    \/ Crash
    \/ \E v \in Foreign : OtherWrites(v)

TypeOK ==
    /\ sys \in Values /\ truth \in Foreign /\ original \in Values
    /\ saved \in BOOLEAN /\ byUs \in BOOLEAN /\ whole \in BOOLEAN
    /\ written \subseteq Targets
    /\ pc \in {"off", "claim", "keep", "keep2", "write", "on",
               "release", "restore", "forget", "forget2"}
    /\ target \in Targets \cup {Nobody}
    /\ seen \in Values \cup {Nobody}

(***************************************************************************)
(* 약속: hanyeong 이 60번에 써 넣은 값은, 덮어쓰기 직전의 값으로 되돌릴 수 있는   *)
(* 채로만 남는다. 그래서 60번에는 우리가 써 넣은 값 아니면 남이 정한 값만 있다.   *)
(***************************************************************************)

\* 60번의 값은 우리가 써 넣은 것이거나, 남이 마지막으로 정한 그 값이다.
OursOrTheirs == byUs \/ sys = truth

\* 아래 셋은 위 약속이 깨지는 원인을 하나씩 가른다.

\* 우리가 써 넣은 값이 60번에 있는 동안, 되돌릴 값이 남아 있다.
KeptWhileOurs == byUs => saved

\* 표시가 서 있으면 원래 값도 남아 있다(표시를 쓰고 값을 쓰기 전의 한 걸음은 뺀다).
FlagHasValue == (saved /\ pc # "keep2") => whole

\* 남아 있는 원래 값은 우리가 덮어쓰기 직전의 값이다.
KeptIsLatest == (byUs /\ saved /\ whole) => original = truth

=============================================================================
