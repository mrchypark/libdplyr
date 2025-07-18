---
inclusion: always
---

# libdplyr 개발 가이드 문서 구조

이 디렉토리는 libdplyr 프로젝트의 개발 표준과 가이드라인을 정의합니다.

## 문서 구조 및 역할

### 핵심 가이드라인
- **project-guidelines.md**: 프로젝트 전체 개요, 아키텍처, 개발 워크플로우
- **language.md**: 한국어 응답 및 다국어 지원 규칙

### 기술적 표준
- **code-quality.md**: 코드 품질, 리뷰 체크리스트, 의존성 관리
- **testing-standards.md**: 테스트 작성 원칙, 커버리지 목표, 벤치마크
- **error-handling.md**: 에러 타입 설계, 디버깅, 사용자 메시지
- **performance-optimization.md**: 메모리 관리, 파싱 최적화, 프로파일링

### 기능별 가이드
- **sql-dialect-support.md**: SQL 방언 지원, 확장성, 호환성 관리

## 문서 간 관계

```
project-guidelines.md (전체 개요)
├── code-quality.md (품질 표준)
├── testing-standards.md (테스트 표준)
├── error-handling.md (에러 처리)
├── performance-optimization.md (성능 최적화)
├── sql-dialect-support.md (방언 지원)
└── language.md (언어 규칙)
```

## 중복 제거 및 통일된 기준

### 통일된 수치 기준
- **코드 커버리지**: 85% 이상 (핵심 로직 95% 이상)
- **성능 회귀 허용**: 5% 이내
- **벤치마크 기준**: 단순 쿼리 1ms, 복잡한 쿼리 10ms 이하

### 참조 관계
- 성능 관련 세부사항은 `performance-optimization.md` 참조
- 테스트 관련 세부사항은 `testing-standards.md` 참조
- 에러 처리 세부사항은 `error-handling.md` 참조

## 사용 방법

1. 새로운 기능 개발 시: `project-guidelines.md` → 해당 기능별 가이드 참조
2. 코드 리뷰 시: `code-quality.md` 체크리스트 활용
3. 테스트 작성 시: `testing-standards.md` 패턴 참조
4. 성능 이슈 시: `performance-optimization.md` 최적화 기법 적용