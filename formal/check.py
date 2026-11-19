#!/usr/bin/env python3
"""단축키 60번의 저장·복원 순서를 TLC 로 검사하고, 찾은 길을 실제 코드로 다시 걷는다.

    python3 formal/check.py            모델만
    python3 formal/check.py --replay   모델, 실제 코드 재생, 고친 곳을 되돌려 보기

끝나는 코드: 0 약속을 지킨다 / 1 약속이 깨지거나 결과가 기록(coverage.json)과 다르다 /
2 판정하지 못했다(덮는 코드가 바뀌어 모델과 다시 맞춰 봐야 하거나, 도구가 없다).
"""
import argparse
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
FORMAL = ROOT / "formal"
COVERAGE = FORMAL / "coverage.json"
TLC_VERSION = "1.7.4"
TLC_URL = f"https://github.com/tlaplus/tlaplus/releases/download/v{TLC_VERSION}/tla2tools.jar"
TLC_SHA256 = "936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
PASS, FAIL, BLOCKED = 0, 1, 2


class Blocked(Exception):
    """판정하지 못했다."""


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def drifted(coverage):
    """기록한 뒤로 내용이 바뀐 파일들."""
    return [path for path, recorded in coverage["files"].items() if sha256(ROOT / path) != recorded]


def tools():
    java = os.environ.get("JAVA") or shutil.which("java")
    if not java:
        raise Blocked("java 가 없습니다. 설치하거나 JAVA 에 실행 파일 경로를 주세요")
    jar = Path(os.environ.get("TLA2TOOLS", ROOT / f".cache/formal-tools/tla2tools-{TLC_VERSION}.jar"))
    if not jar.exists():
        jar.parent.mkdir(parents=True, exist_ok=True)
        print(f"TLC {TLC_VERSION} 을 받습니다: {TLC_URL}", file=sys.stderr)
        try:
            urllib.request.urlretrieve(TLC_URL, jar)
        except OSError as error:
            raise Blocked(f"TLC 를 받지 못했습니다: {error}") from error
    if sha256(jar) != TLC_SHA256:
        raise Blocked(f"TLC {TLC_VERSION} 의 체크섬이 다릅니다: {jar}")
    return java, jar


def tlc(java, jar, model, config, rule, work):
    """규칙 하나만 켜고 TLC 를 돌린다. (holds|breaks, 상태 수, 깨지는 길)"""
    folder = work / f"{config.removesuffix('.cfg')}-{rule}"
    folder.mkdir(parents=True)
    shutil.copy(FORMAL / "models" / model, folder)
    lines = (FORMAL / "models" / config).read_text().splitlines()
    kept = [line for line in lines if not line.startswith("INVARIANT")]
    (folder / "one.cfg").write_text("\n".join([*kept, f"INVARIANT {rule}", ""]))
    run = subprocess.run(
        [java, "-XX:+UseParallelGC", "-cp", str(jar), "tlc2.TLC", "-workers", "1", "-fp", "0",
         "-config", "one.cfg", model],
        cwd=folder, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=300, check=False)
    (folder / "tlc.out").write_text(run.stdout)
    counts = re.findall(r"([\d,]+) distinct states found", run.stdout)
    states = int(counts[-1].replace(",", "")) if counts else 0
    path = re.findall(r"^State \d+: <(\w+)", run.stdout, flags=re.MULTILINE)
    if f"Invariant {rule} is violated" in run.stdout:
        return "breaks", states, path
    if run.returncode == 0 and "Model checking completed. No error has been found." in run.stdout:
        return "holds", states, path
    raise Blocked(f"TLC 가 {config} 의 {rule} 을 판정하지 못했습니다: {folder / 'tlc.out'}")


def model_checks(coverage, work):
    """기록된 모든 (설정, 규칙)을 돌린다. (어긋난 것의 수, 코드가 깨는 규칙들)"""
    java, jar = tools()
    jobs = [(check, rule) for check in coverage["checks"] for rule in check["rules"]]
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        results = list(pool.map(
            lambda job: tlc(java, jar, coverage["model"], job[0]["config"], job[1], work), jobs))
    mismatches, broken = 0, []
    for (check, rule), (seen, states, path) in zip(jobs, results):
        expected = check["rules"][rule]
        note = "" if seen == expected else f"  <- 기록은 {expected}"
        mismatches += seen != expected
        if check["is_code"] and seen == "breaks":
            broken.append(rule)
        line = f"  {check['config']:24} {rule:14} {seen:6} 상태 {states:>5}{note}"
        print(line + (f"\n      길: {' > '.join(path)}" if seen == "breaks" else ""))
    return mismatches, broken


TEST = ["cargo", "test", "--locked", "-p", "hanyeong-core", "--test", "shortcut60_replay"]
WORKSPACE = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "crates"]


def cargo_test(cwd):
    # 사본마다 자기 target/ 에 빌드한다. 한 폴더를 같이 쓰면 되돌린 코드의 산출물이 다음 실행에 섞인다
    # (2026-11-17 에 겪었다: 지금 코드의 테스트가 되돌린 라이브러리로 돌아 실패했다).
    env = {key: value for key, value in os.environ.items() if key != "CARGO_TARGET_DIR"}
    return subprocess.run(TEST, cwd=cwd, env=env, text=True, stdout=subprocess.PIPE,
                          stderr=subprocess.STDOUT, timeout=900, check=False)


def replay(coverage, work):
    """실제 판단 함수로 길을 다시 걷고, 고친 곳을 하나씩 되돌려 테스트가 잡는지 본다.

    되돌리는 것은 임시 폴더의 사본에서만 한다. 어긋난 것의 수를 돌려준다.
    """
    wrong = 0
    held = cargo_test(ROOT)
    passed = re.search(r"test result: ok\. (\d+) passed; 0 failed; 0 ignored", held.stdout)
    if held.returncode != 0 or not passed:
        print("  지금 코드: 실패\n" + held.stdout[-2500:])
        wrong += 1
    else:
        print(f"  지금 코드: {passed.group(1)}건 통과")
    for number, mutation in enumerate(coverage["mutations"], start=1):
        copy = work / f"mutation-{number}"
        for name in WORKSPACE:
            source = ROOT / name
            if source.is_dir():
                shutil.copytree(source, copy / name)
            else:
                copy.mkdir(parents=True, exist_ok=True)
                shutil.copy(source, copy / name)
        target = copy / mutation["file"]
        text = target.read_text()
        if text.count(mutation["before"]) != 1:
            raise Blocked(f"되돌릴 자리를 찾지 못했습니다(코드가 바뀌었다): {mutation['what']}")
        target.write_text(text.replace(mutation["before"], mutation["after"]))
        run = cargo_test(copy)
        (work / f"mutation-{number}.out").write_text(run.stdout)
        if "error: could not compile" in run.stdout or "error[" in run.stdout:
            raise Blocked(f"되돌린 코드가 빌드되지 않습니다: {work / f'mutation-{number}.out'}")
        caught = run.returncode != 0 and mutation["rule"] + ":" in run.stdout
        print(f"  되돌림 {number} ({mutation['what']}): "
              + (f"{mutation['rule']} 에서 잡힘" if caught else "잡지 못함  <- 검사가 이 버그를 놓친다"))
        wrong += not caught
    return wrong


def accept(coverage):
    coverage["files"] = {path: sha256(ROOT / path) for path in coverage["files"]}
    COVERAGE.write_text(json.dumps(coverage, ensure_ascii=False, indent=2) + "\n")
    print("덮는 파일의 해시를 다시 적었습니다.")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--replay", action="store_true", help="실제 코드로 길을 다시 걷는다(cargo test)")
    parser.add_argument("--while-changing", action="store_true",
                        help="덮는 코드가 바뀌어 있어도 끝까지 돌린다. 통과해도 2 로 끝난다")
    parser.add_argument("--accept-sync", action="store_true",
                        help="모델이 지금 코드를 그대로 본뜬다고 확인한 뒤에만: 덮는 파일의 해시를 다시 적는다")
    parser.add_argument("--out", type=Path, help="TLC 출력을 남길 폴더(기본: 임시 폴더)")
    args = parser.parse_args()
    coverage = json.loads(COVERAGE.read_text())
    if args.accept_sync:
        accept(coverage)
        return PASS
    changed = drifted(coverage)
    if changed:
        print("sync_required: 모델이 덮는 코드가 기록한 뒤로 바뀌었습니다:")
        for path in changed:
            print(f"  {path}")
        print("모델과 재생 테스트가 바뀐 코드를 그대로 본뜨는지 본 뒤 --accept-sync 로 적습니다.")
        if not args.while_changing:
            return BLOCKED
    work = args.out or Path(tempfile.mkdtemp(prefix="hanyeong-formal-"))
    work.mkdir(parents=True, exist_ok=True)
    print(f"모델 {coverage['model']} (TLC {TLC_VERSION}, 출력 {work})")
    mismatches, broken = model_checks(coverage, work)
    if args.replay:
        print("실제 코드 재생 (crates/hanyeong-core/tests/shortcut60_replay.rs)")
        mismatches += replay(coverage, work)
    if mismatches:
        print(f"판정: 기록과 다르거나 잡지 못한 것 {mismatches}건")
        return FAIL
    if broken:
        print(f"판정: 지금 코드가 약속을 깬다(깨지는 규칙: {', '.join(broken)}). "
              "길은 pstack 의 /statechart 로 읽는다")
        return FAIL
    if changed:
        print("판정: 결과는 기록과 같지만, 바뀐 코드를 모델과 맞춰 보기 전이다")
        return BLOCKED
    print("판정: 약속을 지킨다(모델의 범위 안에서)")
    return PASS


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Blocked as error:
        print(f"판정하지 못했습니다: {error}", file=sys.stderr)
        sys.exit(BLOCKED)
