# encoding: ascii-8bit
# frozen-string-literal: false
#
# The module storing Ruby interpreter configurations on building.
#
# This file was created by mkconfig.rb when ruby was built.  It contains
# build information for ruby which is used e.g. by mkmf to build
# compatible native extensions.  Any changes made to this file will be
# lost the next time ruby is built.

module RbConfig
  RUBY_VERSION.start_with?("3.2.") or
    raise "ruby lib version (3.2.1) doesn't match executable version (#{RUBY_VERSION})"

  # Ruby installed directory.
  TOPDIR = File.dirname(__FILE__).chomp!("/lib/ruby/3.2.0/aarch64-linux-android-android")
  # DESTDIR on make install.
  DESTDIR = '' unless defined? DESTDIR
  # The hash configurations stored.
  CONFIG = {}
  CONFIG["DESTDIR"] = DESTDIR
  CONFIG["MAJOR"] = "3"
  CONFIG["MINOR"] = "2"
  CONFIG["TEENY"] = "1"
  CONFIG["PATCHLEVEL"] = "31"
  CONFIG["INSTALL"] = '/usr/bin/install -c'
  CONFIG["EXEEXT"] = ""
  CONFIG["prefix"] = (TOPDIR || DESTDIR + "/Users/alex/Development/cipher/src-tauri/gen/android/runtime/dist/aarch64-linux-android")
  CONFIG["ruby_install_name"] = "$(RUBY_BASE_NAME)"
  CONFIG["RUBY_INSTALL_NAME"] = "$(RUBY_BASE_NAME)"
  CONFIG["RUBY_SO_NAME"] = "$(RUBY_BASE_NAME)"
  CONFIG["exec"] = "exec"
  CONFIG["ruby_pc"] = "ruby-3.2.pc"
  CONFIG["CC_WRAPPER"] = ""
  CONFIG["PACKAGE"] = "ruby"
  CONFIG["BUILTIN_TRANSSRCS"] = " enc/trans/newline.c"
  CONFIG["MKMF_VERBOSE"] = "0"
  CONFIG["MANTYPE"] = "doc"
  CONFIG["vendorarchhdrdir"] = "$(vendorhdrdir)/$(sitearch)"
  CONFIG["sitearchhdrdir"] = "$(sitehdrdir)/$(sitearch)"
  CONFIG["rubyarchhdrdir"] = "$(rubyhdrdir)/$(arch)"
  CONFIG["vendorhdrdir"] = "$(rubyhdrdir)/vendor_ruby"
  CONFIG["sitehdrdir"] = "$(rubyhdrdir)/site_ruby"
  CONFIG["rubyhdrdir"] = "$(includedir)/$(RUBY_VERSION_NAME)"
  CONFIG["RUBY_SEARCH_PATH"] = ""
  CONFIG["UNIVERSAL_INTS"] = ""
  CONFIG["UNIVERSAL_ARCHNAMES"] = ""
  CONFIG["configure_args"] = " '--prefix=/Users/alex/Development/cipher/src-tauri/gen/android/runtime/dist/aarch64-linux-android' '--with-openssl-dir=/opt/homebrew/opt/openssl@3' '--enable-shared' '--with-readline-dir=/opt/homebrew/opt/readline' '--with-gmp-dir=/opt/homebrew/opt/gmp' '--with-ext=openssl,psych,+' '--with-opt-dir=/Users/alex/Development/cipher/src-tauri/gen/android/runtime/deps' '--with-libyaml-dir=/Users/alex/Development/cipher/src-tauri/gen/android/runtime/deps' '--with-libyaml-include=/Users/alex/Development/cipher/src-tauri/gen/android/runtime/deps/include' '--with-libyaml-lib=/Users/alex/Development/cipher/src-tauri/gen/android/runtime/deps/lib' '--with-static-linked-ext' '--disable-install-doc' '--without-gmp' '--host=aarch64-linux-android' '--target=aarch64-linux-android' '--without-ext=dbm,openssl,zlib,readline' 'host_alias=aarch64-linux-android' 'target_alias=aarch64-linux-android' 'AR=/Users/alex/Library/Android/sdk/ndk/27.0.12077973/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-ar' 'CC=/Users/alex/Library/Android/sdk/ndk/27.0.12077973/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android24-clang' 'CXX=/Users/alex/Library/Android/sdk/ndk/27.0.12077973/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android24-clang++' 'LD=/Users/alex/Library/Android/sdk/ndk/27.0.12077973/toolchains/llvm/prebuilt/darwin-x86_64/bin/ld' 'RANLIB=/Users/alex/Library/Android/sdk/ndk/27.0.12077973/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-ranlib' 'STRIP=/Users/alex/Library/Android/sdk/ndk/27.0.12077973/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-strip' 'CFLAGS=-fPIC -O2 -I/Users/alex/Development/cipher/src-tauri/gen/android/runtime/deps/include ' 'LDFLAGS=-fPIC -L/Users/alex/Development/cipher/src-tauri/gen/android/runtime/deps/lib'"
  CONFIG["CONFIGURE"] = "configure"
  CONFIG["vendorarchdir"] = "$(vendorlibdir)/$(sitearch)"
  CONFIG["vendorlibdir"] = "$(vendordir)/$(ruby_version)"
  CONFIG["vendordir"] = "$(rubylibprefix)/vendor_ruby"
  CONFIG["sitearchdir"] = "$(sitelibdir)/$(sitearch)"
  CONFIG["sitelibdir"] = "$(sitedir)/$(ruby_version)"
  CONFIG["sitedir"] = "$(rubylibprefix)/site_ruby"
  CONFIG["rubyarchdir"] = "$(rubylibdir)/$(arch)"
  CONFIG["rubylibdir"] = "$(rubylibprefix)/$(ruby_version)"
  CONFIG["ruby_version"] = "3.2.0"
  CONFIG["sitearch"] = "$(arch)"
  CONFIG["arch"] = "aarch64-linux-android-android"
  CONFIG["sitearchincludedir"] = "$(includedir)/$(sitearch)"
  CONFIG["archincludedir"] = "$(includedir)/$(arch)"
  CONFIG["sitearchlibdir"] = "$(libdir)/$(sitearch)"
  CONFIG["archlibdir"] = "$(libdir)/$(arch)"
  CONFIG["libdirname"] = "libdir"
  CONFIG["RUBY_EXEC_PREFIX"] = "/Users/alex/Development/cipher/src-tauri/gen/android/runtime/dist/aarch64-linux-android"
  CONFIG["RUBY_LIB_VERSION"] = ""
  CONFIG["RUBY_LIB_VERSION_STYLE"] = "3\t/* full */"
  CONFIG["RI_BASE_NAME"] = "ri"
  CONFIG["ridir"] = "$(datarootdir)/$(RI_BASE_NAME)"
  CONFIG["rubysitearchprefix"] = "$(rubylibprefix)/$(sitearch)"
  CONFIG["rubyarchprefix"] = "$(rubylibprefix)/$(arch)"
  CONFIG["MAKEFILES"] = "Makefile GNUmakefile"
  CONFIG["PLATFORM_DIR"] = ""
  CONFIG["COROUTINE_TYPE"] = "arm64"
  CONFIG["THREAD_MODEL"] = "pthread"
  CONFIG["SYMBOL_PREFIX"] = ""
  CONFIG["EXPORT_PREFIX"] = ""
  CONFIG["COMMON_HEADERS"] = ""
  CONFIG["COMMON_MACROS"] = ""
  CONFIG["COMMON_LIBS"] = ""
  CONFIG["MAINLIBS"] = "-lz -ldl -lm "
  CONFIG["ENABLE_SHARED"] = "yes"
  CONFIG["DLDSHARED"] = "$(CC) -shared"
  CONFIG["DLDLIBS"] = "-lc"
  CONFIG["SOLIBS"] = "$(MAINLIBS)"
  CONFIG["LIBRUBYARG_SHARED"] = "-Wl,-rpath,$(libdir) -l$(RUBY_SO_NAME)"
  CONFIG["LIBRUBYARG_STATIC"] = "-Wl,-rpath,$(libdir) -l$(RUBY_SO_NAME)-static $(MAINLIBS)"
  CONFIG["LIBRUBYARG"] = "$(LIBRUBYARG_SHARED)"
  CONFIG["LIBRUBY"] = "$(LIBRUBY_SO)"
  CONFIG["LIBRUBY_ALIASES"] = "$(LIBRUBY_SONAME) lib$(RUBY_SO_NAME).$(SOEXT)"
  CONFIG["LIBRUBY_SONAME"] = "lib$(RUBY_SO_NAME).$(SOEXT).$(RUBY_API_VERSION)"
  CONFIG["LIBRUBY_SO"] = "lib$(RUBY_SO_NAME).$(SOEXT).$(RUBY_PROGRAM_VERSION)"
  CONFIG["LIBRUBY_A"] = "lib$(RUBY_SO_NAME)-static.a"
  CONFIG["RUBYW_INSTALL_NAME"] = ""
  CONFIG["rubyw_install_name"] = ""
  CONFIG["EXTDLDFLAGS"] = ""
  CONFIG["EXTLDFLAGS"] = ""
  CONFIG["strict_warnflags"] = ""
  CONFIG["warnflags"] = "-Wall -Wextra -Wextra-tokens -Wdeprecated-declarations -Wdivision-by-zero -Wdiv-by-zero -Wimplicit-function-declaration -Wimplicit-int -Wmisleading-indentation -Wpointer-arith -Wshorten-64-to-32 -Wwrite-strings -Wold-style-definition -Wmissing-noreturn -Wno-cast-function-type -Wno-constant-logical-operand -Wno-long-long -Wno-missing-field-initializers -Wno-overlength-strings -Wno-parentheses-equality -Wno-self-assign -Wno-tautological-compare -Wno-unused-parameter -Wno-unused-value -Wunused-variable -Wundef"
  CONFIG["debugflags"] = "-ggdb3"
  CONFIG["optflags"] = "-O3 -fno-fast-math"
  CONFIG["NULLCMD"] = ":"
  CONFIG["ENABLE_DEBUG_ENV"] = ""
  CONFIG["DLNOBJ"] = "dln.o"
  CONFIG["INSTALL_STATIC_LIBRARY"] = "no"
  CONFIG["YJIT_OBJ"] = ""
  CONFIG["YJIT_LIBS"] = ""
  CONFIG["CARGO_BUILD_ARGS"] = ""
  CONFIG["YJIT_SUPPORT"] = "no"
  CONFIG["CARGO"] = ""
  CONFIG["RUSTC"] = "rustc"
  CONFIG["MJIT_SUPPORT"] = "yes"
  CONFIG["EXECUTABLE_EXTS"] = ""
  CONFIG["ARCHFILE"] = ""
  CONFIG["LIBRUBY_RELATIVE"] = "no"
  CONFIG["EXTOUT"] = ".ext"
  CONFIG["PREP"] = "$(arch)-fake.rb"
  CONFIG["CROSS_COMPILING"] = "yes"
  CONFIG["TEST_RUNNABLE"] = "no"
  CONFIG["rubylibprefix"] = "$(libdir)/$(RUBY_BASE_NAME)"
  CONFIG["setup"] = "Setup"
  CONFIG["SOEXT"] = "so"
  CONFIG["TRY_LINK"] = ""
  CONFIG["PRELOADENV"] = "LD_PRELOAD"
  CONFIG["LIBPATHENV"] = "LD_LIBRARY_PATH"
  CONFIG["RPATHFLAG"] = " -Wl,-rpath,%1$-s"
  CONFIG["LIBPATHFLAG"] = " -L%1$-s"
  CONFIG["LINK_SO"] = ""
  CONFIG["ADDITIONAL_DLDFLAGS"] = ""
  CONFIG["ENCSTATIC"] = "static"
  CONFIG["EXTSTATIC"] = "static"
  CONFIG["ASMEXT"] = "S"
  CONFIG["LIBEXT"] = "a"
  CONFIG["DLEXT"] = "so"
  CONFIG["LDSHAREDXX"] = "$(CXX) -shared"
  CONFIG["LDSHARED"] = "$(CC) -shared"
  CONFIG["CCDLFLAGS"] = "-fPIC"
  CONFIG["STATIC"] = ""
  CONFIG["ARCH_FLAG"] = ""
  CONFIG["DLDFLAGS"] = "-fPIC -L/Users/alex/Development/cipher/src-tauri/gen/android/runtime/deps/lib -Wl,--compress-debug-sections=zlib -L/opt/homebrew/opt/gmp/lib  -Wl,-rpath,/opt/homebrew/opt/gmp/lib -L/Users/alex/Development/cipher/src-tauri/gen/android/runtime/deps/lib  -Wl,-rpath,/Users/alex/Development/cipher/src-tauri/gen/android/runtime/deps/lib"
  CONFIG["ALLOCA"] = ""
  CONFIG["EGREP"] = "/usr/bin/grep -E"
  CONFIG["GREP"] = "/usr/bin/grep"
  CONFIG["dsymutil"] = ""
  CONFIG["codesign"] = ""
  CONFIG["cleanlibs"] = ""
  CONFIG["POSTLINK"] = ":"
  CONFIG["WERRORFLAG"] = "-Werror"
  CONFIG["RUBY_DEVEL"] = ""
  CONFIG["CHDIR"] = "cd -P"
  CONFIG["RMALL"] = "rm -fr"
  CONFIG["RMDIRS"] = "rmdir -p"
  CONFIG["RMDIR"] = "rmdir"
  CONFIG["CP"] = "cp"
  CONFIG["RM"] = "rm -f"
  CONFIG["PKG_CONFIG"] = "pkg-config"
  CONFIG["DOXYGEN"] = ""
  CONFIG["DOT"] = ""
  CONFIG["MKDIR_P"] = "/opt/homebrew/bin/gmkdir -p"
  CONFIG["INSTALL_DATA"] = "$(INSTALL) -m 644"
  CONFIG["INSTALL_SCRIPT"] = "$(INSTALL)"
  CONFIG["INSTALL_PROGRAM"] = "$(INSTALL)"
  CONFIG["SET_MAKE"] = ""
  CONFIG["LN_S"] = "ln -s"
  CONFIG["DLLWRAP"] = ""
  CONFIG["WINDRES"] = ""
  CONFIG["ASFLAGS"] = ""
  CONFIG["ARFLAGS"] = "rcD "
  CONFIG["try_header"] = ""
  CONFIG["CC_VERSION_MESSAGE"] = "Android (12027248, +pgo, -bolt, +lto, -mlgo, based on r522817) clang version 18.0.1 (https://android.googlesource.com/toolchain/llvm-project d8003a456d14a3deb8054cdaa529ffbf02d9b262)\nTarget: aarch64-unknown-linux-android24\nThread model: posix\nInstalledDir: /Users/alex/Library/Android/sdk/ndk/27.0.12077973/toolchains/llvm/prebuilt/darwin-x86_64/bin"
  CONFIG["CC_VERSION"] = "$(CC) --version"
  CONFIG["MJIT_CC"] = "/Users/alex/Library/Android/sdk/ndk/27.0.12077973/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android24-clang"
  CONFIG["CSRCFLAG"] = ""
  CONFIG["COUTFLAG"] = "-o "
  CONFIG["OUTFLAG"] = "-o "
  CONFIG["CPPOUTFILE"] = "-o conftest.i"
  CONFIG["GNU_LD"] = "no"
  CONFIG["GCC"] = "yes"
  CONFIG["CPP"] = "$(CC) -E"
  CONFIG["CXXFLAGS"] = "-fdeclspec"
  CONFIG["OBJEXT"] = "o"
  CONFIG["CPPFLAGS"] = "-I/opt/homebrew/opt/gmp/include -I/Users/alex/Development/cipher/src-tauri/gen/android/runtime/deps/include $(DEFS) $(cppflags)"
  CONFIG["LDFLAGS"] = "-L. -fPIC -L/Users/alex/Development/cipher/src-tauri/gen/android/runtime/deps/lib -fstack-protector-strong -L/opt/homebrew/opt/gmp/lib  -Wl,-rpath,/opt/homebrew/opt/gmp/lib -L/Users/alex/Development/cipher/src-tauri/gen/android/runtime/deps/lib  -Wl,-rpath,/Users/alex/Development/cipher/src-tauri/gen/android/runtime/deps/lib -rdynamic -Wl,-export-dynamic -Wl,--no-as-needed"
  CONFIG["CFLAGS"] = "-fPIC -O2 -I/Users/alex/Development/cipher/src-tauri/gen/android/runtime/deps/include  $(cflags) -fPIC"
  CONFIG["STRIP"] = "/Users/alex/Library/Android/sdk/ndk/27.0.12077973/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-strip -S -x"
  CONFIG["RANLIB"] = "/Users/alex/Library/Android/sdk/ndk/27.0.12077973/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-ranlib"
  CONFIG["OBJDUMP"] = "objdump"
  CONFIG["OBJCOPY"] = ":"
  CONFIG["NM"] = "nm"
  CONFIG["LD"] = "/Users/alex/Library/Android/sdk/ndk/27.0.12077973/toolchains/llvm/prebuilt/darwin-x86_64/bin/ld"
  CONFIG["CXX"] = "/Users/alex/Library/Android/sdk/ndk/27.0.12077973/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android24-clang++"
  CONFIG["AS"] = "as"
  CONFIG["AR"] = "/Users/alex/Library/Android/sdk/ndk/27.0.12077973/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-ar"
  CONFIG["CC"] = "/Users/alex/Library/Android/sdk/ndk/27.0.12077973/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android24-clang"
  CONFIG["wasmoptflags"] = ""
  CONFIG["WASMOPT"] = ""
  CONFIG["target_os"] = "linux-android-android"
  CONFIG["target_vendor"] = "unknown"
  CONFIG["target_cpu"] = "aarch64"
  CONFIG["target"] = "$(target_cpu)-$(target_vendor)-$(target_os)"
  CONFIG["host_os"] = "$(target_os)"
  CONFIG["host_vendor"] = "$(target_vendor)"
  CONFIG["host_cpu"] = "$(target_cpu)"
  CONFIG["host"] = "$(target)"
  CONFIG["build_os"] = "darwin25.0.0"
  CONFIG["build_vendor"] = "apple"
  CONFIG["build_cpu"] = "aarch64"
  CONFIG["build"] = "aarch64-apple-darwin25.0.0"
  CONFIG["RUBY_VERSION_NAME"] = "$(RUBY_BASE_NAME)-$(ruby_version)"
  CONFIG["RUBYW_BASE_NAME"] = "rubyw"
  CONFIG["RUBY_BASE_NAME"] = "ruby"
  CONFIG["RUBY_PROGRAM_VERSION"] = "$(MAJOR).$(MINOR).$(TEENY)"
  CONFIG["RUBY_API_VERSION"] = "$(MAJOR).$(MINOR)"
  CONFIG["HAVE_GIT"] = "yes"
  CONFIG["GIT"] = "git"
  CONFIG["cxxflags"] = ""
  CONFIG["cppflags"] = ""
  CONFIG["cflags"] = "-fdeclspec $(optflags) $(debugflags) $(warnflags)"
  CONFIG["MAKEDIRS"] = "/opt/homebrew/bin/gmkdir -p"
  CONFIG["target_alias"] = "aarch64-linux-android"
  CONFIG["host_alias"] = "$(target_alias)"
  CONFIG["build_alias"] = ""
  CONFIG["LIBS"] = "-lm "
  CONFIG["ECHO_T"] = ""
  CONFIG["ECHO_N"] = ""
  CONFIG["ECHO_C"] = "\\\\c"
  CONFIG["DEFS"] = ""
  CONFIG["mandir"] = "$(datarootdir)/man"
  CONFIG["localedir"] = "$(datarootdir)/locale"
  CONFIG["libdir"] = "$(exec_prefix)/lib"
  CONFIG["psdir"] = "$(docdir)"
  CONFIG["pdfdir"] = "$(docdir)"
  CONFIG["dvidir"] = "$(docdir)"
  CONFIG["htmldir"] = "$(docdir)"
  CONFIG["infodir"] = "$(datarootdir)/info"
  CONFIG["docdir"] = "$(datarootdir)/doc/$(PACKAGE)"
  CONFIG["oldincludedir"] = "/usr/include"
  CONFIG["includedir"] = "$(prefix)/include"
  CONFIG["runstatedir"] = "$(localstatedir)/run"
  CONFIG["localstatedir"] = "$(prefix)/var"
  CONFIG["sharedstatedir"] = "$(prefix)/com"
  CONFIG["sysconfdir"] = "$(prefix)/etc"
  CONFIG["datadir"] = "$(datarootdir)"
  CONFIG["datarootdir"] = "$(prefix)/share"
  CONFIG["libexecdir"] = "$(exec_prefix)/libexec"
  CONFIG["sbindir"] = "$(exec_prefix)/sbin"
  CONFIG["bindir"] = "$(exec_prefix)/bin"
  CONFIG["exec_prefix"] = "$(prefix)"
  CONFIG["PACKAGE_URL"] = ""
  CONFIG["PACKAGE_BUGREPORT"] = ""
  CONFIG["PACKAGE_STRING"] = ""
  CONFIG["PACKAGE_VERSION"] = ""
  CONFIG["PACKAGE_TARNAME"] = ""
  CONFIG["PACKAGE_NAME"] = ""
  CONFIG["PATH_SEPARATOR"] = ":"
  CONFIG["SHELL"] = "/bin/sh"
  CONFIG["UNICODE_VERSION"] = "15.0.0"
  CONFIG["UNICODE_EMOJI_VERSION"] = "15.0"
  CONFIG["platform"] = "$(arch)"
  CONFIG["archdir"] = "$(rubyarchdir)"
  CONFIG["topdir"] = File.dirname(__FILE__)
  # Almost same with CONFIG. MAKEFILE_CONFIG has other variable
  # reference like below.
  #
  #   MAKEFILE_CONFIG["bindir"] = "$(exec_prefix)/bin"
  #
  # The values of this constant is used for creating Makefile.
  #
  #   require 'rbconfig'
  #
  #   print <<-END_OF_MAKEFILE
  #   prefix = #{RbConfig::MAKEFILE_CONFIG['prefix']}
  #   exec_prefix = #{RbConfig::MAKEFILE_CONFIG['exec_prefix']}
  #   bindir = #{RbConfig::MAKEFILE_CONFIG['bindir']}
  #   END_OF_MAKEFILE
  #
  #   => prefix = /usr/local
  #      exec_prefix = $(prefix)
  #      bindir = $(exec_prefix)/bin  MAKEFILE_CONFIG = {}
  #
  # RbConfig.expand is used for resolving references like above in rbconfig.
  #
  #   require 'rbconfig'
  #   p RbConfig.expand(RbConfig::MAKEFILE_CONFIG["bindir"])
  #   # => "/usr/local/bin"
  MAKEFILE_CONFIG = {}
  CONFIG.each{|k,v| MAKEFILE_CONFIG[k] = v.dup}

  # call-seq:
  #
  #   RbConfig.expand(val)         -> string
  #   RbConfig.expand(val, config) -> string
  #
  # expands variable with given +val+ value.
  #
  #   RbConfig.expand("$(bindir)") # => /home/foobar/all-ruby/ruby19x/bin
  def RbConfig::expand(val, config = CONFIG)
    newval = val.gsub(/\$\$|\$\(([^()]+)\)|\$\{([^{}]+)\}/) {
      var = $&
      if !(v = $1 || $2)
	'$'
      elsif key = config[v = v[/\A[^:]+(?=(?::(.*?)=(.*))?\z)/]]
	pat, sub = $1, $2
	config[v] = false
	config[v] = RbConfig::expand(key, config)
	key = key.gsub(/#{Regexp.quote(pat)}(?=\s|\z)/n) {sub} if pat
	key
      else
	var
      end
    }
    val.replace(newval) unless newval == val
    val
  end
  CONFIG.each_value do |val|
    RbConfig::expand(val)
  end

  # call-seq:
  #
  #   RbConfig.fire_update!(key, val)               -> array
  #   RbConfig.fire_update!(key, val, mkconf, conf) -> array
  #
  # updates +key+ in +mkconf+ with +val+, and all values depending on
  # the +key+ in +mkconf+.
  #
  #   RbConfig::MAKEFILE_CONFIG.values_at("CC", "LDSHARED") # => ["gcc", "$(CC) -shared"]
  #   RbConfig::CONFIG.values_at("CC", "LDSHARED")          # => ["gcc", "gcc -shared"]
  #   RbConfig.fire_update!("CC", "gcc-8")                  # => ["CC", "LDSHARED"]
  #   RbConfig::MAKEFILE_CONFIG.values_at("CC", "LDSHARED") # => ["gcc-8", "$(CC) -shared"]
  #   RbConfig::CONFIG.values_at("CC", "LDSHARED")          # => ["gcc-8", "gcc-8 -shared"]
  #
  # returns updated keys list, or +nil+ if nothing changed.
  def RbConfig.fire_update!(key, val, mkconf = MAKEFILE_CONFIG, conf = CONFIG) # :nodoc:
    return if mkconf[key] == val
    mkconf[key] = val
    keys = [key]
    deps = []
    begin
      re = Regexp.new("\\$\\((?:%1$s)\\)|\\$\\{(?:%1$s)\\}" % keys.join('|'))
      deps |= keys
      keys.clear
      mkconf.each {|k,v| keys << k if re =~ v}
    end until keys.empty?
    deps.each {|k| conf[k] = mkconf[k].dup}
    deps.each {|k| expand(conf[k])}
    deps
  end

  # call-seq:
  #
  #   RbConfig.ruby -> path
  #
  # returns the absolute pathname of the ruby command.
  def RbConfig.ruby
    File.join(
      RbConfig::CONFIG["bindir"],
      RbConfig::CONFIG["ruby_install_name"] + RbConfig::CONFIG["EXEEXT"]
    )
  end
end
CROSS_COMPILING = nil unless defined? CROSS_COMPILING
