cmake_minimum_required(VERSION 3.16)
project(gungraun_standalone_example LANGUAGES C)

add_executable(workload workload.c)
target_compile_features(workload PRIVATE c_std_11)
if(CMAKE_C_COMPILER_ID MATCHES "GNU|Clang")
  target_compile_options(workload PRIVATE -g)
endif()
