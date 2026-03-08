fn main() {
  // Set FFmpeg library paths
  println!("cargo:rustc-link-search=native=E:/TPlayer/ffmpeg/ffmpeg-master-latest-win64-gpl-shared/lib");
  println!("cargo:rustc-link-lib=dylib=avcodec");
  println!("cargo:rustc-link-lib=dylib=avformat");
  println!("cargo:rustc-link-lib=dylib=avutil");
  println!("cargo:rustc-link-lib=dylib=swresample");
  
  tauri_build::build()
}
