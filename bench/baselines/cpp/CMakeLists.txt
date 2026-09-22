cmake_minimum_required(VERSION 3.26)

project(hgl_baselines LANGUAGES CXX)

# Built against an installed hgraph, with the compiler and the optimisation
# level hgraph itself was built with. See bench/README.md.
find_package(hgraph CONFIG REQUIRED)

add_executable(hgl_baseline scenarios.cpp)
target_compile_features(hgl_baseline PRIVATE cxx_std_23)
target_link_libraries(hgl_baseline PRIVATE hgraph::core)

add_executable(hgl_fixed_baseline fixed.cpp)
target_compile_features(hgl_fixed_baseline PRIVATE cxx_std_23)
target_link_libraries(hgl_fixed_baseline PRIVATE hgraph::core)
