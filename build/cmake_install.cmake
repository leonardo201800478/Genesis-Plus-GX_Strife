# Install script for directory: C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/core/cd_hw/libchdr

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
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib" TYPE STATIC_LIBRARY OPTIONAL FILES "C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/build/chdr.lib")
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/include/libchdr" TYPE FILE FILES
    "C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/core/cd_hw/libchdr/include/libchdr/bitstream.h"
    "C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/core/cd_hw/libchdr/include/libchdr/cdrom.h"
    "C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/core/cd_hw/libchdr/include/libchdr/chd.h"
    "C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/core/cd_hw/libchdr/include/libchdr/chdconfig.h"
    "C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/core/cd_hw/libchdr/include/libchdr/coretypes.h"
    "C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/core/cd_hw/libchdr/include/libchdr/flac.h"
    "C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/core/cd_hw/libchdr/include/libchdr/huffman.h"
    )
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib/pkgconfig" TYPE FILE FILES "C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/build/libchdr.pc")
endif()

if(NOT CMAKE_INSTALL_LOCAL_ONLY)
  # Include the install script for each subdirectory.
  include("C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/build/tests/cmake_install.cmake")

endif()

string(REPLACE ";" "\n" CMAKE_INSTALL_MANIFEST_CONTENT
       "${CMAKE_INSTALL_MANIFEST_FILES}")
if(CMAKE_INSTALL_LOCAL_ONLY)
  file(WRITE "C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/build/install_local_manifest.txt"
     "${CMAKE_INSTALL_MANIFEST_CONTENT}")
endif()
if(CMAKE_INSTALL_COMPONENT)
  if(CMAKE_INSTALL_COMPONENT MATCHES "^[a-zA-Z0-9_.+-]+$")
    set(CMAKE_INSTALL_MANIFEST "install_manifest_${CMAKE_INSTALL_COMPONENT}.txt")
  else()
    string(MD5 CMAKE_INST_COMP_HASH "${CMAKE_INSTALL_COMPONENT}")
    set(CMAKE_INSTALL_MANIFEST "install_manifest_${CMAKE_INST_COMP_HASH}.txt")
    unset(CMAKE_INST_COMP_HASH)
  endif()
else()
  set(CMAKE_INSTALL_MANIFEST "install_manifest.txt")
endif()

if(NOT CMAKE_INSTALL_LOCAL_ONLY)
  file(WRITE "C:/Users/leonardo.paiva/source/repos/leonardo201800478/Genesis-Plus-GX_Strife/build/${CMAKE_INSTALL_MANIFEST}"
     "${CMAKE_INSTALL_MANIFEST_CONTENT}")
endif()
