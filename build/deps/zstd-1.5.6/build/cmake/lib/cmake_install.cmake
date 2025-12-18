# Install script for directory: C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/core/cd_hw/libchdr/deps/zstd-1.5.6/build/cmake/lib

# Set the install prefix
if(NOT DEFINED CMAKE_INSTALL_PREFIX)
  set(CMAKE_INSTALL_PREFIX "C:/Program Files (x86)/chdr")
endif()
string(REGEX REPLACE "/$" "" CMAKE_INSTALL_PREFIX "${CMAKE_INSTALL_PREFIX}")

# Set the install configuration name.
if(NOT DEFINED CMAKE_INSTALL_CONFIG_NAME)
  if(BUILD_TYPE)
    string(REGEX REPLACE "^[^A-Za-z0-9_]+" ""
           CMAKE_INSTALL_CONFIG_NAME "${BUILD_TYPE}")
  else()
    set(CMAKE_INSTALL_CONFIG_NAME "Debug")
  endif()
  message(STATUS "Install configuration: \"${CMAKE_INSTALL_CONFIG_NAME}\"")
endif()

# Set the component getting installed.
if(NOT CMAKE_INSTALL_COMPONENT)
  if(COMPONENT)
    message(STATUS "Install component: \"${COMPONENT}\"")
    set(CMAKE_INSTALL_COMPONENT "${COMPONENT}")
  else()
    set(CMAKE_INSTALL_COMPONENT)
  endif()
endif()

# Is this installation the result of a crosscompile?
if(NOT DEFINED CMAKE_CROSSCOMPILING)
  set(CMAKE_CROSSCOMPILING "FALSE")
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib/pkgconfig" TYPE FILE FILES "C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/build/deps/zstd-1.5.6/build/cmake/lib/libzstd.pc")
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/include" TYPE FILE FILES
    "C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/core/cd_hw/libchdr/deps/zstd-1.5.6/build/cmake/../../lib/zdict.h"
    "C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/core/cd_hw/libchdr/deps/zstd-1.5.6/build/cmake/../../lib/zstd.h"
    "C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/core/cd_hw/libchdr/deps/zstd-1.5.6/build/cmake/../../lib/zstd_errors.h"
    )
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib" TYPE STATIC_LIBRARY FILES "C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/build/deps/zstd-1.5.6/build/cmake/lib/zstd_static.lib")
endif()

string(REPLACE ";" "\n" CMAKE_INSTALL_MANIFEST_CONTENT
       "${CMAKE_INSTALL_MANIFEST_FILES}")
if(CMAKE_INSTALL_LOCAL_ONLY)
  file(WRITE "C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/build/deps/zstd-1.5.6/build/cmake/lib/install_local_manifest.txt"
     "${CMAKE_INSTALL_MANIFEST_CONTENT}")
endif()
