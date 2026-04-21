# 아티팩트 패키징 가이드

이 문서는 libdplyr DuckDB 확장의 아티팩트 패키징 프로세스에 대한 가이드입니다.

## 개요

**요구사항 R4-AC3**에 따라 다음과 같은 패키징 기능을 제공합니다:
- 플랫폼별 확장 바이너리 패키징
- 버전 정보 및 호환성 메타데이터 포함
- 압축 및 체크섬 생성
- 자동화된 검증 시스템

## 지원 플랫폼

### 현재 지원 플랫폼
- **Linux x86_64**: Ubuntu, CentOS, Debian 등
- **macOS x86_64**: Intel 기반 Mac
- **macOS ARM64**: Apple Silicon Mac (M1/M2)
- **Windows x86_64**: Windows 10/11

### 플랫폼별 특징
| 플랫폼 | 확장자 | 패키지 형식 | 체크섬 도구 |
|--------|--------|-------------|-------------|
| Linux | `.so` | tar.gz | sha256sum |
| macOS | `.dylib` | tar.gz | shasum |
| Windows | `.dll` | zip | certutil |

## 패키징 스크립트

### 개별 플랫폼 패키징
```bash
# 현재 플랫폼용 패키징
./scripts/package-artifacts.sh

# 특정 플랫폼 지정
PLATFORM_OVERRIDE=linux-x86_64 ./scripts/package-artifacts.sh

# 버전 지정
VERSION=v1.0.0 ./scripts/package-artifacts.sh
```

### 멀티플랫폼 패키징
```bash
# 모든 사용 가능한 플랫폼 패키징
./scripts/package-all-platforms.sh

# 특정 패키지 디렉토리 사용
PACKAGE_DIR=release ./scripts/package-all-platforms.sh
```

### Windows 패키징
```cmd
REM Windows에서 패키징
scripts\package-artifacts.bat

REM 버전 지정
set VERSION=v1.0.0
scripts\package-artifacts.bat
```

## 패키지 구조

### 개별 플랫폼 패키지
```
packages/v1.0.0/linux-x86_64/
├── dplyr-linux-x86_64.duckdb_extension  # 확장 바이너리
├── metadata.json                        # 빌드 메타데이터
├── INSTALL.md                           # 설치 가이드
└── checksums.txt                        # 파일 체크섬
```

### 통합 패키지
```
packages/v1.0.0/
├── linux-x86_64/                       # 개별 플랫폼 패키지
├── macos-x86_64/
├── macos-arm64/
├── windows-x86_64/
├── combined/                            # 통합 패키지
│   ├── linux-x86_64/
│   ├── macos-x86_64/
│   ├── macos-arm64/
│   ├── windows-x86_64/
│   ├── install.sh                       # 자동 설치 스크립트
│   ├── install.bat                      # Windows 설치 스크립트
│   └── release-metadata.json           # 릴리스 메타데이터
├── dplyr-v1.0.0-all-platforms.tar.gz   # 통합 아카이브
├── dplyr-v1.0.0-all-platforms.zip      # Windows 호환 아카이브
└── RELEASE_NOTES.md                     # 릴리스 노트
```

## 메타데이터 형식

### metadata.json
```json
{
  "extension": {
    "name": "dplyr",
    "version": "v1.0.0",
    "platform": "linux",
    "architecture": "x86_64",
    "platform_arch": "linux-x86_64",
    "filename": "dplyr-linux-x86_64.duckdb_extension",
    "size_bytes": 2048576,
    "size_human": "2.0M"
  },
  "build": {
    "timestamp": "2024-01-15T10:30:00Z",
    "git_commit": "abc123def456",
    "git_branch": "main",
    "git_tag": "v1.0.0",
    "build_type": "Release"
  },
  "versions": {
    "libdplyr": "0.3.3",
    "rust": "rustc 1.75.0",
    "cmake": "cmake version 3.20.0",
    "duckdb_tested": "v0.10.0"
  },
  "compatibility": {
    "duckdb_min_version": "0.9.0",
    "duckdb_max_version": "1.0.0",
    "abi_version": "1",
    "api_version": "1"
  },
  "features": {
    "dplyr_keywords": true,
    "table_functions": true,
    "error_handling": true,
    "caching": true,
    "debug_logging": true
  },
  "requirements": {
    "minimum_memory_mb": 64,
    "recommended_memory_mb": 256,
    "disk_space_mb": 10
  }
}
```

### release-metadata.json (통합 패키지)
```json
{
  "release": {
    "version": "v1.0.0",
    "extension_name": "dplyr",
    "build_timestamp": "2024-01-15T10:30:00Z",
    "git_commit": "abc123def456",
    "git_branch": "main"
  },
  "platforms": {
    "linux-x86_64": {
      "platform": "linux",
      "architecture": "x86_64",
      "extension_file": "dplyr-linux-x86_64.duckdb_extension",
      "available": true
    },
    "windows-x86_64": {
      "platform": "windows",
      "architecture": "x86_64",
      "extension_file": "dplyr-windows-x86_64.duckdb_extension",
      "available": false,
      "reason": "Build artifacts not found"
    }
  },
  "statistics": {
    "total_platforms": 4,
    "packaged_platforms": 3,
    "missing_platforms": 1,
    "success_rate": "75%"
  }
}
```

## 체크섬 및 보안

### 체크섬 생성
```bash
# SHA256 체크섬 (Linux/macOS)
sha256sum dplyr-linux-x86_64.duckdb_extension > checksums.txt

# Windows
certutil -hashfile dplyr-windows-x86_64.duckdb_extension SHA256 >> checksums.txt
```

### 체크섬 검증
```bash
# Linux/macOS
sha256sum -c checksums.txt

# Windows
certutil -hashfile extension.duckdb_extension SHA256
```

### 보안 고려사항
- 모든 바이너리에 SHA256 체크섬 제공
- 아카이브 파일에도 별도 체크섬 생성
- 빌드 환경 정보 메타데이터에 포함
- Git 커밋 해시로 소스 추적 가능

## 패키지 검증

### 자동 검증
```bash
# 전체 패키지 검증
./scripts/verify-packages.sh

# 특정 버전 검증
VERSION=v1.0.0 ./scripts/verify-packages.sh
```

### 검증 항목
1. **구조 검증**: 필수 파일 존재 확인
2. **무결성 검증**: 체크섬 일치 확인
3. **메타데이터 검증**: JSON 형식 및 필수 필드 확인
4. **확장 파일 검증**: 파일 크기 및 타입 확인
5. **로딩 테스트**: DuckDB에서 확장 로딩 테스트
6. **아카이브 검증**: 압축 파일 무결성 확인

### 검증 리포트
```markdown
# Package Verification Report

**Version**: v1.0.0
**Verification Date**: 2024-01-15T10:30:00Z
**Verified Platforms**: 4

## ✅ Verification Results
- Package Structure: ✅
- File Integrity: ✅
- Metadata Validation: ✅
- Extension Files: ✅
- Archive Integrity: ✅

## 📦 Verified Platforms
- linux-x86_64: ✅
- macos-x86_64: ✅
- macos-arm64: ✅
- windows-x86_64: ✅
```

## CI/CD 통합

### GitHub Actions 워크플로우
```yaml
- name: Package Artifacts
  run: |
    ./scripts/package-all-platforms.sh
    ./scripts/verify-packages.sh

- name: Upload Packages
  uses: actions/upload-artifact@v4
  with:
    name: release-packages
    path: packages/
```

### 릴리스 자동화
```yaml
- name: Create Release
  if: github.event_name == 'release'
  run: |
    # 패키징
    ./scripts/package-all-platforms.sh
    
    # 검증
    ./scripts/verify-packages.sh
    
    # GitHub Release에 업로드
    gh release upload ${{ github.event.release.tag_name }} packages/$VERSION/*.tar.gz
    gh release upload ${{ github.event.release.tag_name }} packages/$VERSION/*.zip
```

## 사용자 설치

### 자동 설치
```bash
# 다운로드 및 압축 해제
curl -L https://github.com/org/repo/releases/download/v1.0.0/dplyr-v1.0.0-all-platforms.tar.gz | tar -xz

# 자동 설치 실행
cd combined
./install.sh
```

### 수동 설치
```bash
# 플랫폼별 패키지 다운로드
curl -L -O https://github.com/org/repo/releases/download/v1.0.0/dplyr-v1.0.0-linux-x86_64.tar.gz

# 압축 해제
tar -xzf dplyr-v1.0.0-linux-x86_64.tar.gz

# 체크섬 검증
cd linux-x86_64
sha256sum -c checksums.txt

# DuckDB에서 로드
duckdb -c "LOAD './dplyr-linux-x86_64.duckdb_extension';"
```

## 문제 해결

### 일반적인 문제

#### 빌드 아티팩트 없음
```bash
# 문제: Extension file not found
# 해결: 먼저 확장 빌드
mkdir build && cd build
cmake .. -DCMAKE_BUILD_TYPE=Release
cmake --build . --parallel
```

#### 체크섬 불일치
```bash
# 문제: Checksum mismatch
# 해결: 파일 재다운로드 또는 재빌드
rm -f extension.duckdb_extension
# 다시 빌드 또는 다운로드
```

#### 플랫폼 호환성 문제
```bash
# 문제: Extension fails to load
# 해결: 올바른 플랫폼 패키지 확인
uname -s -m  # 현재 플랫폼 확인
# 해당 플랫폼 패키지 다운로드
```

### 디버깅 도구

#### 패키지 내용 확인
```bash
# 아카이브 내용 확인
tar -tzf dplyr-v1.0.0-all-platforms.tar.gz

# 메타데이터 확인
jq . metadata.json

# 확장 파일 정보
file dplyr-linux-x86_64.duckdb_extension
nm -D dplyr-linux-x86_64.duckdb_extension | grep dplyr
```

#### 로딩 테스트
```bash
# 기본 로딩 테스트
duckdb :memory: -c "LOAD './extension.duckdb_extension'; SELECT 'OK';"

# 디버그 모드
DPLYR_DEBUG=1 duckdb :memory: -c "LOAD './extension.duckdb_extension';"
```

## 개발자 가이드

### 새 플랫폼 추가
1. `package-artifacts.sh`에 플랫폼 감지 로직 추가
2. `package-all-platforms.sh`의 PLATFORMS 배열에 추가
3. CI/CD 워크플로우에 빌드 매트릭스 추가
4. 테스트 및 검증

### 패키징 스크립트 수정
```bash
# 스크립트 테스트
./scripts/package-artifacts.sh
./scripts/verify-packages.sh

# 새 기능 추가 시 검증 스크립트도 업데이트
```

### 메타데이터 스키마 변경
1. `metadata.json` 형식 업데이트
2. 검증 스크립트의 필수 필드 목록 업데이트
3. 문서 업데이트
4. 하위 호환성 고려

## 모범 사례

### 패키징 전 체크리스트
- [ ] 모든 플랫폼에서 빌드 성공
- [ ] 테스트 통과 확인
- [ ] 버전 태그 생성
- [ ] 릴리스 노트 준비

### 품질 보증
- 자동화된 검증 스크립트 사용
- 여러 DuckDB 버전에서 테스트
- 체크섬 검증 필수
- 메타데이터 정확성 확인

### 배포 전략
- 단계적 롤아웃 (베타 → 안정 버전)
- 이전 버전과의 호환성 유지
- 명확한 업그레이드 가이드 제공
- 롤백 계획 준비

이 가이드를 따라 안정적이고 신뢰할 수 있는 패키지를 생성하고 배포하세요.
