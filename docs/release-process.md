# 릴리스 프로세스 가이드

## 개요

이 문서는 DuckDB dplyr 확장의 릴리스 프로세스를 설명합니다. R4-AC3 및 R8-AC3 요구사항에 따라 자동화된 릴리스 시스템을 구축하여 다중 플랫폼 바이너리 배포와 포괄적인 릴리스 노트를 제공합니다.

## 릴리스 유형

### 1. 안정 릴리스 (Stable Release)
- **형식**: `v1.0.0`, `v1.1.0`, `v2.0.0`
- **용도**: 프로덕션 환경에서 사용 가능한 안정적인 버전
- **배포**: GitHub Releases + Community Repository 제출

### 2. 프리릴리스 (Pre-release)
- **형식**: `v1.0.0-beta`, `v1.0.0-rc1`, `v1.0.0-alpha`
- **용도**: 테스트 및 피드백 수집용
- **배포**: GitHub Releases만 (Community Repository 제외)

### 3. 패치 릴리스 (Patch Release)
- **형식**: `v1.0.1`, `v1.0.2`
- **용도**: 버그 수정 및 보안 패치
- **배포**: 자동 배포 (긴급 시)

## 릴리스 프로세스

### Phase 1: 준비 단계

#### 1.1 코드 준비
```bash
# 1. 최신 코드 동기화
git checkout main
git pull origin main

# 2. 브랜치 정리
git branch -d old-feature-branches

# 3. 의존성 업데이트 확인
cargo update
```

#### 1.2 품질 검증
```bash
# 1. 전체 테스트 실행
cargo test --all-features
cargo test --release

# 2. 벤치마크 실행
cargo bench

# 3. 코드 품질 검사
cargo clippy -- -D warnings
cargo fmt --check

# 4. 보안 감사
cargo audit
```

#### 1.3 문서 업데이트
- [ ] CHANGELOG.md 업데이트
- [ ] README.md 버전 정보 확인
- [ ] API 문서 최신화
- [ ] 예제 코드 검증

### Phase 2: 릴리스 생성

#### 2.1 자동 릴리스 (권장)
```bash
# 릴리스 스크립트 사용
./scripts/create-release.sh -v v1.0.0

# 프리릴리스 생성
./scripts/create-release.sh -v v1.0.0-beta -p

# 드래프트 릴리스 생성
./scripts/create-release.sh -v v1.0.0 -d
```

#### 2.2 수동 릴리스
```bash
# 1. 태그 생성
git tag -a v1.0.0 -m "Release v1.0.0"
git push origin v1.0.0

# 2. GitHub Actions 워크플로우 트리거
gh workflow run release-deploy.yml -f tag=v1.0.0
```

### Phase 3: 릴리스 검증

#### 3.1 자동 검증
- ✅ 다중 플랫폼 빌드 성공
- ✅ 스모크 테스트 통과
- ✅ 체크섬 생성 및 검증
- ✅ 다운로드 링크 접근성 확인

#### 3.2 수동 검증
```bash
# 1. 각 플랫폼별 다운로드 테스트
curl -L https://github.com/repo/releases/download/v1.0.0/dplyr-linux-x86_64.duckdb_extension -o test.extension

# 2. 확장 로딩 테스트
duckdb -c "LOAD './test.extension'; SELECT 'OK' as status;"

# 3. 기본 기능 테스트
duckdb -c "
LOAD './test.extension';
DPLYR 'mtcars %>% select(mpg, cyl) %>% filter(mpg > 20)';
"
```

### Phase 4: 배포 후 작업

#### 4.1 Community Repository 제출 (안정 릴리스만)
```bash
# 1. 제출 아티팩트 다운로드
gh run download --name community-submission-v1.0.0

# 2. community-extensions 저장소 포크 및 클론
gh repo fork duckdb/community-extensions
git clone https://github.com/yourusername/community-extensions.git

# 3. 브랜치 생성 및 파일 추가
cd community-extensions
git checkout -b add-dplyr-extension-1.0.0
cp ../community-submission/extension.json extensions/dplyr/

# 4. PR 생성
git add extensions/dplyr/extension.json
git commit -m "Add dplyr extension v1.0.0"
git push origin add-dplyr-extension-1.0.0
gh pr create --title "Add dplyr extension v1.0.0" --body-file ../community-submission/SUBMISSION_INSTRUCTIONS.md
```

#### 4.2 모니터링 및 지원
- [ ] GitHub Issues 모니터링
- [ ] 다운로드 통계 추적
- [ ] 사용자 피드백 수집
- [ ] 버그 리포트 대응

## 릴리스 자동화 시스템

### GitHub Actions 워크플로우

#### 1. release.yml
- **트리거**: 릴리스 태그 생성 시
- **기능**: 다중 플랫폼 빌드 및 테스트
- **출력**: 플랫폼별 바이너리 아티팩트

#### 2. release-deploy.yml
- **트리거**: 수동 실행 또는 릴리스 이벤트
- **기능**: GitHub Release 생성 및 배포
- **출력**: 완전한 릴리스 패키지

### 릴리스 아티팩트

#### 필수 파일
- `dplyr-linux-x86_64.duckdb_extension`
- `dplyr-macos-x86_64.duckdb_extension`
- `dplyr-macos-arm64.duckdb_extension`
- `dplyr-windows-x86_64.duckdb_extension`
- `checksums.sha256`
- `install.sh`
- `release-metadata.json`

#### 메타데이터 파일
```json
{
  "version": "1.0.0",
  "release_date": "2024-08-29T12:00:00Z",
  "platforms": [...],
  "duckdb_compatibility": {...},
  "quality_metrics": {...}
}
```

## 버전 관리 정책

### Semantic Versioning
- **MAJOR**: 호환성을 깨는 변경사항
- **MINOR**: 하위 호환성을 유지하는 기능 추가
- **PATCH**: 하위 호환성을 유지하는 버그 수정

### 브랜치 전략
- `main`: 안정적인 코드, 릴리스 준비 상태
- `develop`: 개발 중인 기능들
- `feature/*`: 개별 기능 개발
- `hotfix/*`: 긴급 버그 수정

### 태그 규칙
- 형식: `v{MAJOR}.{MINOR}.{PATCH}[-{PRERELEASE}]`
- 예시: `v1.0.0`, `v1.1.0-beta`, `v1.0.1`

## 롤백 프로세스

### 릴리스 롤백
```bash
# 1. 문제가 있는 릴리스 삭제
gh release delete v1.0.0 --yes

# 2. 태그 삭제
git tag -d v1.0.0
git push origin :refs/tags/v1.0.0

# 3. 이전 버전으로 되돌리기
gh release edit v0.9.0 --latest
```

### 핫픽스 릴리스
```bash
# 1. 핫픽스 브랜치 생성
git checkout -b hotfix/v1.0.1 v1.0.0

# 2. 버그 수정
# ... 코드 수정 ...

# 3. 패치 릴리스 생성
./scripts/create-release.sh -v v1.0.1
```

## 품질 게이트

### 릴리스 전 체크리스트
- [ ] 모든 테스트 통과 (단위, 통합, 스모크)
- [ ] 코드 커버리지 85% 이상
- [ ] 성능 벤치마크 통과
- [ ] 보안 스캔 통과
- [ ] 문서 업데이트 완료
- [ ] CHANGELOG.md 업데이트

### 자동 품질 검사
- **빌드**: 모든 플랫폼에서 성공적 빌드
- **테스트**: 전체 테스트 스위트 통과
- **성능**: 벤치마크 회귀 없음
- **보안**: 취약점 스캔 통과
- **호환성**: DuckDB 버전 호환성 확인

## 문제 해결

### 일반적인 문제

#### 빌드 실패
```bash
# 로그 확인
gh run view --log

# 로컬에서 재현
cargo build --release --target x86_64-unknown-linux-gnu
```

#### 테스트 실패
```bash
# 특정 테스트 실행
cargo test test_name -- --nocapture

# 플랫폼별 테스트
cargo test --target x86_64-pc-windows-msvc
```

#### 배포 실패
```bash
# 워크플로우 재실행
gh run rerun <run-id>

# 수동 배포
gh release create v1.0.0 --title "Release v1.0.0" --notes-file release-notes.md
```

## 연락처 및 지원

### 릴리스 관련 문의
- **GitHub Issues**: 기술적 문제
- **GitHub Discussions**: 일반적인 질문
- **Email**: 긴급한 보안 문제

### 문서 및 리소스
- [GitHub Actions 워크플로우](.github/workflows/)
- [릴리스 스크립트](scripts/create-release.sh)
- [Community 제출 가이드](community-repo-submission.md)
- [품질 보증 가이드](../docs/code-quality.md)