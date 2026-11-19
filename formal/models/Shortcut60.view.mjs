// formal/models/Shortcut60.tla 를 읽는 사람의 말로 옮긴다. pstack 의 /statechart 가 읽는다.

const calling = ["claim", "keep", "keep2", "write"];
const leaving = ["release", "restore", "forget", "forget2"];

// 60번에 지금 누구의 값이 있는가.
const holder = (v) =>
  v.byUs ? "hanyeong 의 F-키" : v.sys === v.truth ? "사용자가 정한 값" : "옛 값으로 덮임";

// 되돌릴 값이 어떤 상태로 남아 있는가.
const kept = (v) =>
  !v.saved
    ? "되돌릴 값 없음"
    : !v.whole
      ? "표시만 있고 되돌릴 값 없음"
      : v.original === v.truth
        ? "되돌릴 값 있음"
        : "되돌릴 값이 낡음";

const phase = (v) =>
  v.pc === "on" ? "켜짐" : v.pc === "off" ? "꺼짐" : calling.includes(v.pc) ? "켜는 중" : "끄는 중";

export const state = (v) => [phase(v), `60번: ${holder(v)} · ${kept(v)}`];

export const group = phase;

export const colors = { 켜짐: "teal", 꺼짐: "tan", "켜는 중": "blue", "끄는 중": "purple" };

const steps = {
  Start: "hanyeong 을 띄운다",
  StartRestore: "restore 를 돌린다",
  ClaimRead: "60번을 읽는다",
  KeepFlag: "바꿔 둔다는 표시를 남긴다",
  KeepValue: "원래 값을 남긴다",
  KeepRecord: "써 넣을 키와 원래 값을 남긴다",
  ClaimWrite: "60번을 F-키로 쓴다",
  Quit: "종료한다",
  ReleaseRead: "60번을 읽는다",
  Restore: "원래 값을 60번에 쓴다",
  ForgetFlag: "표시를 지운다",
  ForgetValue: "남긴 값을 지운다",
  ForgetRecord: "남긴 것을 지운다",
  Crash: "강제로 죽는다",
  OtherWrites: "남이 60번을 바꾼다",
};

export function step(action, from, to) {
  if (action === "KeepFlag" && from.saved) return "표시가 이미 있어 원래 값을 다시 남기지 않는다";
  if (action === "StartRestore" && from.byUs && from.sys !== to.target)
    return "다른 F-키를 주고 restore 를 돌린다";
  return steps[action] ?? action;
}

export const title = "단축키 60번: 바꾸고 되돌리기";
