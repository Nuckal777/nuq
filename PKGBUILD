# Maintainer: Erik "Nuckal777" Schubert <nuckal777+nuq@gmail.com>
pkgname=nuq
pkgver=0.1.2
pkgrel=1
pkgdesc="A multi-format frontend to jq"
arch=('x86_64')
url="https://github.com/Nuckal777/nuq"
license=('Unlicense')
depends=('gcc-libs' 'jq')
makedepends=('cargo' 'git')
source=("$pkgname"::"git+https://github.com/Nuckal777/nuq#tag=v0.1.2")
noextract=()
md5sums=('SKIP')

check() {
    cd "$pkgname"
    JQ_LIB_DIR=/lib RUSTUP_TOOLCHAIN=stable cargo test --release --locked --target-dir=target
}

build() {
    cd "$pkgname"
    JQ_LIB_DIR=/lib RUSTUP_TOOLCHAIN=stable cargo build --release --locked --all-features --target-dir=target
}

package() {
    cd "$pkgname"
    install -Dm 755 target/release/${pkgname} -t "${pkgdir}/usr/bin"
}
