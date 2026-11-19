#!/bin/sh
# hanyeong 전용 서명 인증서를 로그인 키체인에 만든다. 한 번만 돌린다.
#
#   tools/setup_signing.sh --create
#
# 임시 서명한 앱은 접근성 권한이 빌드 내용에 묶여 다시 설치할 때마다 풀린다. 같은 인증서로
# 서명하면 권한이 "이 인증서로 서명한 dev.hanyeong" 에 묶여 다시 설치해도 유지된다.
#
# 하는 일: 자체 서명 인증서(코드 서명 전용, 10년)와 개인 키를 만들어 로그인 키체인에 넣고,
# codesign 만 그 키를 쓰게 허용한다. 개인 키는 키체인 밖에 남기지 않는다. 시스템의 신뢰 설정은
# 바꾸지 않는다: 이 인증서는 다른 Mac 에서나 Gatekeeper 에게는 아무 의미가 없다.
# 지우려면 키체인 접근 앱에서 "hanyeong Local Signing" 인증서와 키를 지운다.
#
# gksdud 의 signing/setup-local-signing.sh 를 본으로 삼았다.
set -eu
identity='hanyeong Local Signing'
keychain="$HOME/Library/Keychains/login.keychain-db"
if [ "${1:-}" != --create ]; then
    sed -n 2,14p "$0"
    exit 1
fi
if security find-certificate -c "$identity" "$keychain" >/dev/null 2>&1; then
    echo "이미 있습니다: $identity. 바꾸지 않습니다." >&2
    exit 1
fi
umask 077
stage=$(mktemp -d /private/tmp/hanyeong-signing.XXXXXX)
trap 'rm -rf "$stage"' EXIT
cat >"$stage/openssl.cnf" <<CNF
[req]
distinguished_name = subject
x509_extensions = code_signing
prompt = no

[subject]
CN = $identity

[code_signing]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
subjectKeyIdentifier = hash
CNF
openssl req -new -x509 -newkey rsa:3072 -nodes -days 3650 -sha256 \
    -config "$stage/openssl.cnf" -keyout "$stage/private.key" -out "$stage/certificate.pem" 2>/dev/null
# 키체인으로 옮기는 동안에만 쓰는 암호. 어디에도 남기지 않는다.
HANYEONG_P12_PASSWORD=$(openssl rand -hex 24)
export HANYEONG_P12_PASSWORD
openssl pkcs12 -export -inkey "$stage/private.key" -in "$stage/certificate.pem" \
    -name "$identity" -out "$stage/identity.p12" -passout env:HANYEONG_P12_PASSWORD
security import "$stage/identity.p12" -k "$keychain" -P "$HANYEONG_P12_PASSWORD" -T /usr/bin/codesign
unset HANYEONG_P12_PASSWORD
echo "만들었습니다: $identity"
openssl x509 -in "$stage/certificate.pem" -noout -fingerprint -sha1
