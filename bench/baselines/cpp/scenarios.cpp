// C++ baselines for the hgl runtime's benchmark scenarios (docs/decisions/0002).
//
// Each scenario here has a Rust twin that must build the same graph, give the
// same checksum, and run within 5% of the time measured for this program.
// Nodes are plain static nodes, not library operators, so that what is timed
// is the runtime -- scheduling, notification, reading and writing a
// time-series -- and not operator dispatch.
//
// Every run checks its result against a closed form, so a timing is never
// reported for a graph that computed the wrong thing.

#include <hgraph/runtime/runtime.h>
#include <hgraph/types/graph_wiring.h>
#include <hgraph/types/metadata/type_registry.h>
#include <hgraph/types/static_node.h>

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <string>
#include <string_view>
#include <utility>

namespace
{
    using namespace hgraph;
    using I64 = std::int64_t;
    using U64 = std::uint64_t;

    // Graph shape, read by compose(). Set once from the command line.
    I64 g_cycles = 0;
    I64 g_depth  = 0;
    I64 g_width  = 0;

    // Written by the sink. Evaluation is single-threaded.
    U64 g_checksum = 0;
    U64 g_evals    = 0;

    /** Emits 0, 1, 2, ... one value per engine cycle, for ``count`` cycles. */
    struct Pulse
    {
        static constexpr auto name              = "pulse";
        static constexpr bool schedule_on_start = true;
        static void           eval(NodeScheduler sched, Scalar<"count", I64> count, State<I64> emitted,
                                   Out<TS<I64>> out)
        {
            const I64 n = emitted.get();
            out.set(n);
            emitted.set(n + 1);
            if (n + 1 < count.value()) { sched.schedule(MIN_TD); }
        }
    };

    struct AddOne
    {
        static constexpr auto name = "add_one";
        static void           eval(In<"in", TS<I64>> in, Out<TS<I64>> out) { out.set(in.value() + 1); }
    };

    struct AddConst
    {
        static constexpr auto name = "add_const";
        static void           eval(In<"in", TS<I64>> in, Scalar<"k", I64> k, Out<TS<I64>> out)
        {
            out.set(in.value() + k.value());
        }
    };

    struct Add
    {
        static constexpr auto name = "add";
        static void           eval(In<"lhs", TS<I64>> lhs, In<"rhs", TS<I64>> rhs, Out<TS<I64>> out)
        {
            out.set(lhs.value() + rhs.value());
        }
    };

    struct Checksum
    {
        static constexpr auto name = "checksum";
        static void           eval(In<"in", TS<I64>> in)
        {
            g_checksum += static_cast<U64>(in.value());
            ++g_evals;
        }
    };

    Port<TS<I64>> chain(Wiring &w, Port<TS<I64>> x, I64 depth)
    {
        for (I64 i = 0; i < depth; ++i) { x = wire<AddOne>(w, x); }
        return x;
    }

    /** pulse -> add_one -> checksum. The smallest graph: cost per cycle. */
    struct TickGraph
    {
        static constexpr auto name = "tick";
        static void           compose(Wiring &w) { wire<Checksum>(w, chain(w, wire<Pulse>(w, g_cycles), 1)); }
    };

    /** pulse -> add_one x depth -> checksum. Every node evaluates every cycle. */
    struct ChainGraph
    {
        static constexpr auto name = "chain";
        static void compose(Wiring &w) { wire<Checksum>(w, chain(w, wire<Pulse>(w, g_cycles), g_depth)); }
    };

    /** pulse fans out to ``width`` chains of ``depth``, folded by add. */
    struct WideChainGraph
    {
        static constexpr auto name = "wide_chain";
        static void           compose(Wiring &w)
        {
            auto          src   = wire<Pulse>(w, g_cycles);
            Port<TS<I64>> total = chain(w, wire<AddConst>(w, src, I64{0}), g_depth);
            for (I64 branch = 1; branch < g_width; ++branch)
            {
                total = wire<Add>(w, total, chain(w, wire<AddConst>(w, src, branch), g_depth));
            }
            wire<Checksum>(w, total);
        }
    };

    struct Expectation
    {
        U64 checksum;
        U64 nodes;
    };

    // Sum over i in [0, n) of i.
    U64 triangle(U64 n) { return n * (n - 1) / 2; }

    Expectation expect_chain(U64 n, U64 depth) { return {triangle(n) + n * depth, depth + 2}; }

    Expectation expect_wide(U64 n, U64 depth, U64 width)
    {
        const U64 per_cycle_constant = triangle(width) + width * depth;
        return {width * triangle(n) + n * per_cycle_constant, 1 + width * (1 + depth) + (width - 1) + 1};
    }

    template <typename G>
    double run(I64 cycles)
    {
        GraphBuilder         graph_builder = build_graph<G>();
        GraphExecutorBuilder executor_builder;
        executor_builder.graph_builder(std::move(graph_builder))
            .start_time(MIN_ST)
            .end_time(MIN_ST + TimeDelta{cycles + 8});
        GraphExecutorValue executor = executor_builder.make_executor();
        auto               view     = executor.view();

        const auto begin = std::chrono::steady_clock::now();
        view.run();
        const auto end = std::chrono::steady_clock::now();
        return std::chrono::duration<double>(end - begin).count();
    }

    I64 option(int argc, char **argv, std::string_view flag, I64 fallback)
    {
        for (int i = 2; i + 1 < argc; ++i)
        {
            if (flag == argv[i]) { return std::strtoll(argv[i + 1], nullptr, 10); }
        }
        return fallback;
    }
}  // namespace

int main(int argc, char **argv)
{
    if (argc < 2)
    {
        std::fputs("usage: hgl_baseline tick|chain|wide_chain [--cycles N] [--depth D] [--width W]\n", stderr);
        return 2;
    }
    const std::string scenario = argv[1];

    (void)TypeRegistry::instance().register_scalar<I64>("int");

    double      seconds = 0.0;
    Expectation expected{};
    if (scenario == "tick")
    {
        g_cycles = option(argc, argv, "--cycles", 3'000'000);
        g_depth  = 1;
        g_width  = 1;
        expected = expect_chain(static_cast<U64>(g_cycles), 1);
        seconds  = run<TickGraph>(g_cycles);
    }
    else if (scenario == "chain")
    {
        g_cycles = option(argc, argv, "--cycles", 100'000);
        g_depth  = option(argc, argv, "--depth", 100);
        g_width  = 1;
        expected = expect_chain(static_cast<U64>(g_cycles), static_cast<U64>(g_depth));
        seconds  = run<ChainGraph>(g_cycles);
    }
    else if (scenario == "wide_chain")
    {
        g_cycles = option(argc, argv, "--cycles", 20'000);
        g_depth  = option(argc, argv, "--depth", 30);
        g_width  = option(argc, argv, "--width", 30);
        expected = expect_wide(static_cast<U64>(g_cycles), static_cast<U64>(g_depth), static_cast<U64>(g_width));
        seconds  = run<WideChainGraph>(g_cycles);
    }
    else
    {
        std::fprintf(stderr, "unknown scenario '%s'\n", scenario.c_str());
        return 2;
    }

    const bool   ok            = g_checksum == expected.checksum && g_evals == static_cast<U64>(g_cycles);
    const double ns_per_cycle  = seconds * 1e9 / static_cast<double>(g_cycles);
    const double ns_per_node   = ns_per_cycle / static_cast<double>(expected.nodes);
    std::printf("{\"scenario\":\"%s\",\"impl\":\"hgraph-cpp\",\"cycles\":%lld,\"depth\":%lld,\"width\":%lld,"
                "\"nodes\":%llu,\"run_seconds\":%.6f,\"ns_per_cycle\":%.2f,\"ns_per_node_eval\":%.2f,"
                "\"checksum\":%llu,\"expected\":%llu,\"sink_evals\":%llu,\"ok\":%s}\n",
                scenario.c_str(), static_cast<long long>(g_cycles), static_cast<long long>(g_depth),
                static_cast<long long>(g_width), static_cast<unsigned long long>(expected.nodes), seconds,
                ns_per_cycle, ns_per_node, static_cast<unsigned long long>(g_checksum),
                static_cast<unsigned long long>(expected.checksum), static_cast<unsigned long long>(g_evals),
                ok ? "true" : "false");
    return ok ? 0 : 1;
}
