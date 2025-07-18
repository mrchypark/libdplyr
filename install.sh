#!/bin/bash
# libdplyr 설치 스크립트
# 사용법:
#   curl -sSL https://raw.githubusercontent.com/libdplyr/libdplyr/main/install.sh | sh
#   LIBDPLYR_VERSION=v1.0.0 ./install.sh

set -e

# 색상 정의
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 설정 변수
REPO="libdplyr/libdplyr"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
FALLBACK_INSTALL_DIR="$HOME/.local/bin"
VERSION="${LIBDPLYR_VERSION:-latest}"
BINARY_NAME="libdplyr"

# 로깅 함수
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

# 필수 도구 확인
check_dependencies() {
    local missing_deps=()
    
    if ! command -v curl >/dev/null 2>&1; then
        missing_deps+=("curl")
    fi
    
    if ! command -v tar >/dev/null 2>&1 && ! command -v unzip >/dev/null 2>&1; then
        missing_deps+=("tar 또는 unzip")
    fi
    
    if [ ${#missing_deps[@]} -ne 0 ]; then
        log_error "다음 도구들이 필요합니다: ${missing_deps[*]}"
        log_error "패키지 매니저를 사용하여 설치하세요:"
        log_error "  Ubuntu/Debian: sudo apt-get install curl tar"
        log_error "  CentOS/RHEL: sudo yum install curl tar"
        log_error "  macOS: brew install curl (tar는 기본 설치됨)"
        exit 1
    fi
}

# 플랫폼 감지
detect_platform() {
    local os arch
    
    case "$(uname -s)" in
        Linux*)
            os="linux"
            ;;
        Darwin*)
            os="macos"
            ;;
        MINGW*|MSYS*|CYGWIN*)
            os="windows"
            ;;
        *)
            log_error "지원되지 않는 운영체제: $(uname -s)"
            log_error "지원되는 플랫폼: Linux, macOS, Windows"
            exit 1
            ;;
    esac
    
    case "$(uname -m)" in
        x86_64|amd64)
            arch="x86_64"
            ;;
        aarch64|arm64)
            arch="aarch64"
            ;;
        *)
            log_error "지원되지 않는 아키텍처: $(uname -m)"
            log_error "지원되는 아키텍처: x86_64, aarch64"
            exit 1
            ;;
    esac
    
    echo "${os}-${arch}"
}

# 최신 버전 가져오기
get_latest_version() {
    log_info "최신 버전 정보를 가져오는 중..."
    
    local latest_version
    latest_version=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | \
        grep '"tag_name":' | \
        sed -E 's/.*"([^"]+)".*/\1/')
    
    if [ -z "$latest_version" ]; then
        log_error "최신 버전 정보를 가져올 수 없습니다."
        log_error "네트워크 연결을 확인하거나 수동으로 버전을 지정하세요:"
        log_error "  LIBDPLYR_VERSION=v1.0.0 $0"
        exit 1
    fi
    
    echo "$latest_version"
}

# 바이너리 다운로드
download_binary() {
    local version="$1"
    local platform="$2"
    local binary_name="libdplyr-${platform}"
    
    # Windows의 경우 .exe 확장자 추가
    if [ "$platform" = "windows-x86_64" ]; then
        binary_name="${binary_name}.exe"
    fi
    
    local download_url="https://github.com/${REPO}/releases/download/${version}/${binary_name}"
    
    log_info "다운로드 중: ${download_url}"
    
    # 다운로드 시도
    if curl -f -L --progress-bar -o "$BINARY_NAME" "$download_url"; then
        log_success "바이너리 다운로드 완료"
    else
        log_error "바이너리 다운로드 실패"
        log_error "다음을 확인해주세요:"
        log_error "  1. 네트워크 연결 상태"
        log_error "  2. 지정된 버전이 존재하는지 확인: https://github.com/${REPO}/releases"
        log_error "  3. 플랫폼 지원 여부: ${platform}"
        exit 1
    fi
    
    # 실행 권한 부여
    chmod +x "$BINARY_NAME"
    
    # 바이너리 검증
    if ! ./"$BINARY_NAME" --version >/dev/null 2>&1; then
        log_error "다운로드된 바이너리가 올바르지 않습니다."
        log_error "바이너리가 손상되었거나 플랫폼과 호환되지 않을 수 있습니다."
        exit 1
    fi
    
    log_success "바이너리 검증 완료"
}

# 설치 디렉토리 준비
prepare_install_dir() {
    local install_path="$1"
    local install_dir
    install_dir=$(dirname "$install_path")
    
    if [ ! -d "$install_dir" ]; then
        log_info "설치 디렉토리 생성: $install_dir"
        if ! mkdir -p "$install_dir" 2>/dev/null; then
            return 1
        fi
    fi
    
    return 0
}

# 바이너리 설치
install_binary() {
    local install_path="$1"
    local use_sudo="$2"
    
    log_info "설치 위치: $install_path"
    
    if [ "$use_sudo" = "true" ]; then
        if ! command -v sudo >/dev/null 2>&1; then
            log_error "sudo 명령을 찾을 수 없습니다."
            log_error "관리자 권한으로 직접 실행하거나 다른 설치 위치를 사용하세요."
            return 1
        fi
        
        log_info "관리자 권한이 필요합니다. sudo를 사용합니다..."
        if sudo cp "$BINARY_NAME" "$install_path"; then
            sudo chmod +x "$install_path"
            return 0
        else
            return 1
        fi
    else
        if cp "$BINARY_NAME" "$install_path"; then
            return 0
        else
            return 1
        fi
    fi
}

# PATH 확인 및 안내
check_path() {
    local install_dir="$1"
    
    if echo "$PATH" | grep -q "$install_dir"; then
        return 0
    else
        return 1
    fi
}

# 설치 후 검증
verify_installation() {
    local binary_name="$1"
    
    if command -v "$binary_name" >/dev/null 2>&1; then
        log_success "설치 검증 완료!"
        log_info "설치된 버전:"
        "$binary_name" --version
        return 0
    else
        return 1
    fi
}

# PATH 설정 안내
show_path_instructions() {
    local install_dir="$1"
    
    log_warning "libdplyr이 PATH에서 찾을 수 없습니다."
    log_info "다음 중 하나를 실행하여 PATH를 업데이트하세요:"
    echo
    echo "# Bash 사용자:"
    echo "echo 'export PATH=\"\$PATH:$install_dir\"' >> ~/.bashrc"
    echo "source ~/.bashrc"
    echo
    echo "# Zsh 사용자:"
    echo "echo 'export PATH=\"\$PATH:$install_dir\"' >> ~/.zshrc"
    echo "source ~/.zshrc"
    echo
    echo "# 현재 세션에서만 사용:"
    echo "export PATH=\"\$PATH:$install_dir\""
    echo
}

# 사용법 안내
show_usage() {
    echo
    log_success "libdplyr 설치가 완료되었습니다!"
    echo
    log_info "기본 사용법:"
    echo "  libdplyr --help                    # 도움말 보기"
    echo "  echo \"select(name)\" | libdplyr    # stdin에서 입력 받기"
    echo "  libdplyr -i query.R -o result.sql # 파일에서 읽고 파일로 저장"
    echo "  libdplyr -t \"select(name, age)\"   # 직접 코드 입력"
    echo
    log_info "고급 옵션:"
    echo "  libdplyr -d mysql                 # MySQL 방언 사용"
    echo "  libdplyr --pretty                 # 예쁜 형식으로 출력"
    echo "  libdplyr --json                   # JSON 형식으로 출력"
    echo "  libdplyr --validate-only          # 문법 검증만 수행"
    echo
    log_info "더 많은 정보: https://github.com/${REPO}"
}

# 메인 함수
main() {
    log_info "libdplyr 설치를 시작합니다..."
    
    # 의존성 확인
    check_dependencies
    
    # 플랫폼 감지
    local platform
    platform=$(detect_platform)
    log_info "감지된 플랫폼: $platform"
    
    # 버전 결정
    if [ "$VERSION" = "latest" ]; then
        VERSION=$(get_latest_version)
        log_info "최신 버전: $VERSION"
    else
        log_info "지정된 버전: $VERSION"
    fi
    
    # 임시 디렉토리 생성 및 이동
    local temp_dir
    temp_dir=$(mktemp -d)
    local original_dir
    original_dir=$(pwd)
    
    # 정리 함수 등록
    cleanup() {
        cd "$original_dir"
        rm -rf "$temp_dir"
    }
    trap cleanup EXIT
    
    cd "$temp_dir"
    
    # 바이너리 다운로드
    download_binary "$VERSION" "$platform"
    
    # 설치 시도
    local install_success=false
    local final_install_path=""
    
    # 1차 시도: 기본 설치 디렉토리
    if prepare_install_dir "$INSTALL_DIR/$BINARY_NAME"; then
        if install_binary "$INSTALL_DIR/$BINARY_NAME" false 2>/dev/null; then
            install_success=true
            final_install_path="$INSTALL_DIR/$BINARY_NAME"
            log_success "설치 완료: $final_install_path"
        elif install_binary "$INSTALL_DIR/$BINARY_NAME" true 2>/dev/null; then
            install_success=true
            final_install_path="$INSTALL_DIR/$BINARY_NAME"
            log_success "설치 완료 (sudo 사용): $final_install_path"
        fi
    fi
    
    # 2차 시도: 폴백 설치 디렉토리
    if [ "$install_success" = false ]; then
        log_warning "$INSTALL_DIR에 설치할 수 없습니다. 사용자 디렉토리에 설치를 시도합니다..."
        
        if prepare_install_dir "$FALLBACK_INSTALL_DIR/$BINARY_NAME"; then
            if install_binary "$FALLBACK_INSTALL_DIR/$BINARY_NAME" false; then
                install_success=true
                final_install_path="$FALLBACK_INSTALL_DIR/$BINARY_NAME"
                log_success "설치 완료: $final_install_path"
            fi
        fi
    fi
    
    # 설치 실패 시 오류 처리
    if [ "$install_success" = false ]; then
        log_error "설치에 실패했습니다."
        log_error "다음을 시도해보세요:"
        log_error "  1. 관리자 권한으로 실행: sudo $0"
        log_error "  2. 사용자 정의 설치 위치: INSTALL_DIR=~/bin $0"
        log_error "  3. 수동 설치: 바이너리를 직접 다운로드하여 PATH에 추가"
        exit 1
    fi
    
    # 설치 검증
    if verify_installation "$BINARY_NAME"; then
        show_usage
    else
        # PATH 설정 안내
        local install_dir
        install_dir=$(dirname "$final_install_path")
        if ! check_path "$install_dir"; then
            show_path_instructions "$install_dir"
        fi
        
        log_info "설치는 완료되었지만 PATH 설정이 필요할 수 있습니다."
        log_info "새 터미널을 열거나 위의 PATH 설정 명령을 실행한 후 다시 시도하세요."
    fi
}

# 스크립트가 직접 실행될 때만 main 함수 호출
if [ "${BASH_SOURCE[0]}" = "${0}" ]; then
    main "$@"
fi