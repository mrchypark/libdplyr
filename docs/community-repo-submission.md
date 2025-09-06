# DuckDB Community Repository 등록 가이드

## 개요

이 문서는 dplyr 확장을 DuckDB Community Repository에 등록하기 위한 준비 과정과 요구사항을 설명합니다.

## Community Repository 등록 요구사항

### 1. 기본 요구사항 (R4-AC3, R8-AC3 충족)

#### 확장 메타데이터
- **이름**: dplyr
- **설명**: R dplyr syntax support for DuckDB
- **버전**: Semantic versioning (예: 1.0.0)
- **라이선스**: MIT 또는 Apache-2.0
- **저장소**: GitHub 공개 저장소

#### 지원 플랫폼
- Linux x86_64
- macOS x86_64 (Intel)
- macOS ARM64 (Apple Silicon)
- Windows x86_64

#### DuckDB 호환성
- 최소 버전: 0.9.0
- 최대 버전: 1.0.0 (테스트됨)
- ABI 호환성: 보장됨

### 2. 기술적 요구사항

#### 빌드 시스템
- CMake 기반 빌드
- 자동화된 CI/CD 파이프라인
- 다중 플랫폼 빌드 지원

#### 테스트 커버리지
- 단위 테스트: 85% 이상
- 통합 테스트: 포함
- 스모크 테스트: 자동화됨
- 성능 테스트: 벤치마크 포함

#### 문서화
- README.md: 설치 및 사용법
- API 문서: 완전한 함수 참조
- 예제: 실용적인 사용 사례
- 변경 로그: 버전별 변경사항

### 3. 품질 보증

#### 코드 품질
- Rust clippy: 모든 경고 해결
- 코드 포맷팅: rustfmt 적용
- 보안 스캔: 취약점 없음
- 의존성 감사: 최신 상태

#### 성능 기준
- 단순 쿼리 변환: <2ms (P95)
- 복잡 쿼리 변환: <15ms (P95)
- 확장 로딩: <50ms (P95)
- 메모리 안정성: 누수 없음

## 등록 준비 체크리스트

### Phase 1: 기본 준비
- [ ] 공개 GitHub 저장소 설정
- [ ] MIT/Apache-2.0 라이선스 적용
- [ ] README.md 작성 완료
- [ ] CHANGELOG.md 유지
- [ ] 기본 문서화 완료

### Phase 2: 기술적 준비
- [ ] 다중 플랫폼 빌드 검증
- [ ] CI/CD 파이프라인 안정화
- [ ] 자동화된 테스트 통과
- [ ] 성능 벤치마크 달성
- [ ] 보안 스캔 통과

### Phase 3: 품질 검증
- [ ] 코드 리뷰 완료
- [ ] 외부 테스터 피드백 수집
- [ ] 문서 검토 완료
- [ ] 예제 코드 검증
- [ ] 호환성 테스트 완료

### Phase 4: 제출 준비
- [ ] extension.json 메타데이터 파일 생성
- [ ] 릴리스 노트 작성
- [ ] 커뮤니티 가이드라인 준수 확인
- [ ] 제출 PR 준비

## extension.json 메타데이터 예시

```json
{
  "name": "dplyr",
  "description": "R dplyr syntax support for DuckDB - transpile dplyr pipelines to SQL",
  "version": "1.0.0",
  "language": "C++/Rust",
  "build": "cmake",
  "license": "MIT",
  "maintainers": [
    {
      "name": "Your Name",
      "email": "your.email@example.com",
      "github": "yourusername"
    }
  ],
  "repository": {
    "github": "yourusername/libdplyr",
    "ref": "main"
  },
  "docs": {
    "readme": "README.md",
    "changelog": "CHANGELOG.md",
    "examples": "examples/"
  },
  "platforms": [
    {
      "name": "linux_amd64",
      "file": "dplyr-linux-x86_64.duckdb_extension"
    },
    {
      "name": "osx_amd64", 
      "file": "dplyr-macos-x86_64.duckdb_extension"
    },
    {
      "name": "osx_arm64",
      "file": "dplyr-macos-arm64.duckdb_extension"
    },
    {
      "name": "windows_amd64",
      "file": "dplyr-windows-x86_64.duckdb_extension"
    }
  ],
  "duckdb_version": {
    "min": "0.9.0",
    "max": "1.0.0"
  },
  "dependencies": [],
  "tags": ["dplyr", "r", "data-manipulation", "transpiler", "sql"],
  "install": {
    "load": "LOAD 'dplyr';",
    "usage": "DPLYR 'mtcars %>% select(mpg, cyl) %>% filter(mpg > 20)';"
  }
}
```

## 제출 프로세스

### 1. 사전 검토
1. 모든 체크리스트 항목 완료 확인
2. 내부 품질 검토 수행
3. 베타 테스터 피드백 수집
4. 문서 최종 검토

### 2. 제출 준비
1. extension.json 파일 생성
2. 최신 릴리스 태그 생성
3. 모든 플랫폼 바이너리 검증
4. 체크섬 파일 생성

### 3. Community Repository 제출
1. DuckDB community-extensions 저장소 포크
2. 새 브랜치 생성: `add-dplyr-extension`
3. extension.json 파일 추가
4. PR 생성 및 제출
5. 리뷰 프로세스 참여

### 4. 승인 후 작업
1. 커뮤니티 피드백 모니터링
2. 버그 리포트 대응
3. 정기적인 업데이트 제공
4. DuckDB 버전 호환성 유지

## 유지보수 가이드라인

### 정기 업데이트
- **보안 패치**: 즉시 적용
- **버그 수정**: 2주 이내
- **기능 추가**: 분기별 검토
- **DuckDB 호환성**: 새 버전 출시 시 검증

### 커뮤니티 지원
- GitHub Issues 모니터링
- 사용자 질문 응답
- 문서 개선 지속
- 예제 코드 업데이트

### 품질 유지
- CI/CD 파이프라인 유지
- 테스트 커버리지 모니터링
- 성능 회귀 방지
- 보안 스캔 정기 실행

## 참고 자료

- [DuckDB Extension Development Guide](https://duckdb.org/docs/extensions/overview)
- [Community Extensions Repository](https://github.com/duckdb/community-extensions)
- [Extension Submission Guidelines](https://duckdb.org/docs/extensions/community_extensions)
- [DuckDB Extension Template](https://github.com/duckdb/extension-template)

## 연락처

Community Repository 등록 관련 문의:
- DuckDB Discord: #extensions 채널
- GitHub Issues: community-extensions 저장소
- 이메일: extensions@duckdb.org