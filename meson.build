project('arkana', 'cpp',
  version: '0.2.0',
  meson_version: '>=1.1.0',
  default_options: [
    'cpp_std=c++23',
    'warning_level=2',
    'optimization=2',
  ],
)

slint_src = meson.project_source_root() / 'subprojects' / 'slint-1.17.1'
slint_build = meson.project_build_root() / 'slint-cpp-build'
slint_prefix = meson.project_build_root() / 'slint-prefix'

# ── Build slint-cpp static library ───────────────────────────────────
slint_lib = custom_target(
  'slint-cpp',
  output: 'slint-cpp-stamp',
  command: ['sh', '-c',
    'cmake -S ' + slint_src + ' -B ' + slint_build
    + ' -GNinja'
    + ' -DCMAKE_BUILD_TYPE=Release'
    + ' -DBUILD_SHARED_LIBS=OFF'
    + ' -DSLINT_BUILD_RUNTIME=ON'
    + ' -DSLINT_FEATURE_COMPILER=OFF'
    + ' -DSLINT_FEATURE_BACKEND_WINIT=OFF'
    + ' -DSLINT_FEATURE_BACKEND_WINIT_X11=OFF'
    + ' -DSLINT_FEATURE_BACKEND_WINIT_WAYLAND=ON'
    + ' -DSLINT_FEATURE_RENDERER_FEMTOVG=ON'
    + ' -DSLINT_FEATURE_RENDERER_SOFTWARE=OFF'
    + ' -DSLINT_FEATURE_BACKEND_LINUXKMS=ON'
    + ' -DSLINT_FEATURE_FONTCONFIG=OFF'
    + ' -DSLINT_FEATURE_ACCESSIBILITY=OFF'
    + ' -DSLINT_FEATURE_INTERPRETER=OFF'
    + ' -DSLINT_FEATURE_TESTING=OFF'
    + ' -DSLINT_FEATURE_GETTEXT=OFF'
    + ' && ninja -C ' + slint_build + ' api/cpp/libslint_cpp.a'
    + ' && mkdir -p ' + slint_prefix / 'lib' + ' ' + slint_prefix / 'include'
    + ' && cp ' + slint_build / 'api/cpp' / 'libslint_cpp.a' + ' ' + slint_prefix / 'lib/'
    + ' && cp -r ' + slint_src / 'api/cpp/include/*' + ' ' + slint_prefix / 'include/'
    + ' && cp -r ' + slint_build / 'api/cpp/generated_include/*' + ' ' + slint_prefix / 'include/'
    + ' && touch @OUTPUT@'
  ],
)

# ── Resolve transitive static dependencies ───────────────────────────
slint_cc = meson.get_compiler('cpp')
transitive_deps = []
foreach lib : ['input', 'udev', 'gbm', 'drm', 'seat', 'xkbcommon', 'wayland-client', 'fontconfig', 'freetype']
  dep = slint_cc.find_library(lib, required: false)
  if dep.found()
    transitive_deps += dep
  endif
endforeach

slint_dep = declare_dependency(
  link_args: ['-L' + slint_prefix / 'lib', '-lslint_cpp'],
  dependencies: transitive_deps,
  compile_args: ['-I' + slint_prefix / 'include'],
)

deps = [slint_dep]

cpp_args = ['-fno-exceptions']

if get_option('alsa')
  alsa_dep = dependency('alsa', required: false)
  if alsa_dep.found()
    deps += alsa_dep
    cpp_args += ['-DARKANA_HAS_ALSA']
  else
    message('ALSA not found, building without audio support')
  endif
endif

# ── Build slint-compiler (Rust host tool) ────────────────────────────
slint_compiler_build = meson.project_build_root() / 'slint-compiler-cargo'

slint_compiler = custom_target(
  'slint-compiler',
  output: 'slint-compiler',
  command: ['sh', '-c',
    'cargo build --release -p slint-compiler'
    + ' --manifest-path ' + slint_src / 'tools' / 'compiler' / 'Cargo.toml'
    + ' --target-dir ' + slint_compiler_build
    + ' && cp ' + slint_compiler_build / 'release' / 'slint-compiler'
    + ' @OUTPUT@'
  ],
)

# ── Compile .slint UI files ──────────────────────────────────────────
main_window_ui = custom_target(
  'main_window_ui',
  input: 'ui/main_window.slint',
  output: 'main_window.slint.h',
  command: [slint_compiler, '-o', '@OUTPUT@', '-I', meson.project_source_root() / 'ui', '-I', meson.project_source_root() / 'assets', '@INPUT@'],
  depends: slint_lib,
)

arkana_sources = files(
  'src/main.cpp',
  'src/entries.cpp',
  'src/covers.cpp',
  'src/joypad.cpp',
  'src/launch.cpp',
  'src/settings.cpp',
  'src/terminal.cpp',
  'src/audio.cpp',
)

arkana = executable('arkana',
  arkana_sources,
  main_window_ui,
  dependencies: deps,
  cpp_args: cpp_args,
  include_directories: include_directories('src'),
  install: true,
)
