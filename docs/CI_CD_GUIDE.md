# CI/CD 시스템 가이드

이 문서는 libdplyr 프로젝트의 CI/CD 시스템에 대한 종합적인 가이드입니다.

## 목차

- [개요](#개요)
- [워크플로우 구조](#워크플로우-구조)
- [설정 및 사용법](#설정-및-사용법)
- [트러블슈팅](#트러블슈팅)
- [기여자 가이드](#기여자-가이드)
- [고급 설정](#고급-설정)

## 개요

libdplyr 프로젝트는 GitHub Actions를 기반으로 한 포괄적인 CI/CD 시스템을 사용합니다. 이 시스템은 다음과 같은 기능을 제공합니다:

- 🔍 **자동화된 코드 품질 검사** (포맷팅, 린팅, 테스트)
- 🛡️ **보안 감사** (의존성 취약점 검사)
- 📊 **성능 모니터링** (벤치마크, 회귀 감지)
- 📈 **코드 커버리지** 측정 및 리포팅
- 🚀 **자동 릴리스** (태그 기반 배포)
- 🔄 **의존성 자동 관리** (Dependabot 통합)
- 📱 **알림 시스템** (실패 시 자동 이슈 생성)

## 워크플로우 구조

### 주요 워크플로우

#### 1. CI 워크플로우 (`.github/workflows/ci.yml`)

메인 CI 파이프라인으로 모든 코드 변경 시 실행됩니다.

**트리거:**
- `main`, `develop` 브랜치에 push
- Pull Request 생성/업데이트

**작업 단계:**
1. **Check** - 기본 문법 검사
2. **Test Suite** - 다중 플랫폼 테스트
3. **Format Check** - 코드 포맷팅 검증
4. **Clippy** - 린팅 검사
5. **Documentation** - 문서 빌드 검증
6. **Coverage** - 코드 커버리지 측정
7. **Benchmarks** - 성능 벤치마크
8. **Security** - 보안 감사
9. **Monitoring** - CI 메트릭 수집

#### 2. 최적화된 CI 워크플로우 (`.github/workflows/ci-optimized.yml`)

성능 최적화된 CI 파이프라인으로 변경 사항에 따라 조건부 실행됩니다.

**최적화 기능:**
- 파일 변경 감지 기반 조건부 실행
- 병렬 처리 최적화
- 캐시 전략 개선
- 리소스 사용량 최적화

#### 3. 릴리스 워크플로우 (`.github/workflows/release.yml`)

Git 태그 기반 자동 릴리스 시스템입니다.

**트리거:**
- `v*.*.*` 형식의 태그 push
- 수동 트리거 (workflow_dispatch)

**릴리스 프로세스:**
1. 릴리스 검증 (버전 형식, Cargo.toml 일치성)
2. 사전 릴리스 테스트 (전체 테스트 스위트)
3. 다중 플랫폼 바이너리 빌드
4. GitHub Release 생성
5. crates.io 게시 (안정 버전만)

#### 4. 알림 워크플로우 (`.github/workflows/notification.yml`)

CI 실패/성공 시 자동 알림 및 이슈 관리 시스템입니다.

**기능:**
- CI 실패 시 자동 이슈 생성
- 상세한 실패 분석 리포트
- Slack 알림 (설정 시)
- 성공 시 관련 이슈 자동 종료

#### 5. Dependabot 자동 병합 (`.github/workflows/dependabot-auto-merge.yml`)

의존성 업데이트 자동 검증 및 병합 시스템입니다.

**자동 병합 조건:**
- 보안 업데이트 (브레이킹 체인지 없음)
- 패치 업데이트 (5개 이하 의존성)
- 마이너 업데이트 (단일 의존성)

## 설정 및 사용법

### 필수 설정

#### 1. GitHub Secrets

다음 시크릿을 GitHub 저장소에 설정해야 합니다:

```bash
# crates.io 게시용 (선택사항)
CRATES_IO_TOKEN=your_crates_io_token

# Slack 알림용 (선택사항)
SLACK_WEBHOOK_URL=your_slack_webhook_url

# Codecov 업로드용 (선택사항)
CODECOV_TOKEN=your_codecov_token
```

#### 2. 브랜치 보호 규칙

`main` 브랜치에 다음 보호 규칙을 설정하는 것을 권장합니다:

- Require status checks to pass before merging
- Require branches to be up to date before merging
- Required status checks:
  - `Check`
  - `Test Suite`
  - `Rustfmt`
  - `Clippy`

### 로컬 개발 환경 설정

#### 1. 필수 도구 설치

```bash
# Rust 설치 (rustup 사용)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 필수 컴포넌트 설치
rustup component add rustfmt clippy

# 추가 도구 설치 (선택사항)
cargo install cargo-audit cargo-deny cargo-tarpaulin
```

#### 2. 사전 커밋 검사

커밋 전에 다음 명령어로 로컬 검사를 실행하세요:

```bash
# 포맷팅 검사 및 수정
cargo fmt

# 린팅 검사
cargo clippy --all-features -- -D warnings

# 테스트 실행
cargo test --all-features

# 보안 감사 (선택사항)
cargo audit
```

### CI 작업 실행 조건

#### 자동 실행 조건

| 작업 | main/develop push | PR | 조건 |
|------|-------------------|----|----|
| Check | ✅ | ✅ | 항상 |
| Test Suite | ✅ | ✅ | 항상 |
| Format/Clippy | ✅ | ✅ | 항상 |
| Coverage | ✅ | ✅ | main/develop만 |
| Benchmarks | ✅ | ❌ | main만 |
| Security | ✅ | ✅ | 주간 또는 보안 관련 변경 |

#### 수동 실행

GitHub Actions 탭에서 워크플로우를 수동으로 실행할 수 있습니다:

1. Actions 탭 이동
2. 실행할 워크플로우 선택
3. "Run workflow" 버튼 클릭

## 트러블슈팅

### 일반적인 문제 및 해결책

#### 1. 빌드 실패

**증상:** `cargo build` 또는 `cargo test` 실패

**해결책:**
```bash
# 의존성 업데이트
cargo update

# 캐시 정리
cargo clean

# 로컬에서 재현
cargo build --all-features
cargo test --all-features
```

#### 2. 포맷팅 오류

**증상:** Rustfmt 검사 실패

**해결책:**
```bash
# 자동 포맷팅 적용
cargo fmt

# 변경사항 커밋
git add .
git commit -m "Fix formatting"
```

#### 3. Clippy 경고

**증상:** Clippy 린팅 검사 실패

**해결책:**
```bash
# Clippy 경고 확인
cargo clippy --all-features -- -D warnings

# 경고 수정 후 재실행
cargo clippy --all-features --fix
```

#### 4. 테스트 실패

**증상:** 단위 테스트 또는 통합 테스트 실패

**해결책:**
```bash
# 특정 테스트 실행
cargo test test_name -- --nocapture

# 테스트 로그 확인
RUST_LOG=debug cargo test

# 테스트 병렬성 조정
cargo test -- --test-threads=1
```

#### 5. 보안 감사 실패

**증상:** cargo-audit에서 취약점 발견

**해결책:**
```bash
# 취약점 확인
cargo audit

# 의존성 업데이트
cargo update

# 대체 크레이트 검토
cargo tree -d
```

#### 6. 커버리지 측정 실패

**증상:** cargo-tarpaulin 실행 실패

**해결책:**
```bash
# 로컬에서 커버리지 실행
cargo tarpaulin --all-features --workspace

# 타임아웃 증가
cargo tarpaulin --timeout 300

# 특정 테스트 제외
cargo tarpaulin --skip-clean --ignore-tests
```

### CI 캐시 문제

#### 캐시 무효화

캐시 관련 문제가 발생하면 다음 방법으로 해결할 수 있습니다:

1. **Cargo.lock 업데이트:** 의존성 변경 시 자동으로 캐시가 무효화됩니다.
2. **수동 캐시 정리:** GitHub Actions 캐시 페이지에서 수동으로 삭제 가능합니다.
3. **캐시 키 수정:** `.github/actions/setup-rust-cache/action.yml`에서 캐시 키 로직을 수정할 수 있습니다.

### 성능 최적화

#### CI 실행 시간 단축

1. **조건부 실행 활용:** 최적화된 CI 워크플로우 사용
2. **병렬 처리:** 테스트 매트릭스 최적화
3. **캐시 최적화:** 효율적인 캐시 전략 사용
4. **불필요한 작업 스킵:** 변경 사항에 따른 조건부 실행

## 기여자 가이드

### Pull Request 워크플로우

1. **브랜치 생성**
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **로컬 테스트**
   ```bash
   cargo fmt
   cargo clippy --all-features -- -D warnings
   cargo test --all-features
   ```

3. **커밋 및 푸시**
   ```bash
   git add .
   git commit -m "Add your feature"
   git push origin feature/your-feature-name
   ```

4. **Pull Request 생성**
   - GitHub에서 PR 생성
   - CI 검사 통과 확인
   - 리뷰 요청

### 코드 품질 가이드라인

#### 1. 테스트 작성

- 모든 공개 함수에 대한 테스트 작성
- 에지 케이스 및 에러 케이스 테스트
- 통합 테스트로 전체 워크플로우 검증

#### 2. 문서화

- 공개 API에 대한 문서 주석 작성
- 예제 코드 포함
- README 업데이트

#### 3. 성능 고려사항

- 벤치마크 테스트 추가
- 메모리 사용량 최적화
- 알고리즘 복잡도 고려

### 릴리스 프로세스

#### 1. 버전 업데이트

```bash
# Cargo.toml에서 버전 업데이트
version = "1.2.3"

# 변경사항 커밋
git add Cargo.toml
git commit -m "Bump version to 1.2.3"
```

#### 2. 태그 생성 및 푸시

```bash
# 태그 생성
git tag v1.2.3

# 태그 푸시 (자동 릴리스 트리거)
git push origin v1.2.3
```

#### 3. 릴리스 확인

- GitHub Actions에서 릴리스 워크플로우 확인
- GitHub Releases 페이지에서 릴리스 확인
- crates.io에서 게시 확인 (안정 버전)

## 고급 설정

### 커스텀 워크플로우 작성

새로운 워크플로우를 추가하려면:

1. `.github/workflows/` 디렉토리에 YAML 파일 생성
2. 적절한 트리거 및 작업 정의
3. 기존 액션 재사용 (`setup-rust-cache` 등)

### 환경별 설정

#### 개발 환경

```yaml
env:
  RUST_LOG: debug
  CARGO_INCREMENTAL: 1
```

#### 프로덕션 환경

```yaml
env:
  RUST_LOG: info
  CARGO_INCREMENTAL: 0
```

### 모니터링 및 알림 커스터마이징

#### Slack 통합

1. Slack 웹훅 URL 생성
2. GitHub Secrets에 `SLACK_WEBHOOK_URL` 추가
3. 알림 워크플로우가 자동으로 Slack 메시지 전송

#### 이메일 알림

GitHub 저장소 설정에서 이메일 알림을 구성할 수 있습니다.

### 보안 설정

#### 1. 의존성 보안

- Dependabot 자동 업데이트 활성화
- 보안 감사 정기 실행
- 취약점 발견 시 즉시 대응

#### 2. 시크릿 관리

- 민감한 정보는 GitHub Secrets 사용
- 환경별 시크릿 분리
- 정기적인 시크릿 로테이션

## 참고 자료

- [GitHub Actions 문서](https://docs.github.com/en/actions)
- [Rust CI/CD 모범 사례](https://doc.rust-lang.org/cargo/guide/continuous-integration.html)
- [Dependabot 설정](https://docs.github.com/en/code-security/dependabot)
- [Codecov 통합](https://docs.codecov.com/docs)

## 지원 및 문의

CI/CD 시스템 관련 문제나 질문이 있으면:

1. [Issues](../../issues) 페이지에서 기존 이슈 검색
2. 새로운 이슈 생성 (적절한 라벨 사용)
3. [Discussions](../../discussions)에서 질문 및 토론

---

*이 문서는 CI/CD 시스템 변경 시 함께 업데이트됩니다.*