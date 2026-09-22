// Paired TSL[TSB] execution and alternating whole REF routes.
#include <hgraph/runtime/runtime.h>
#include <hgraph/types/static_node.h>
#include <hgraph/types/graph_wiring.h>
#include <hgraph/types/metadata/type_registry.h>
#include <hgraph/lib/std/operators/collection.h>
#include <chrono>
#include <cstdio>
#include <string>
using namespace hgraph;
using FixedBundle = UnNamedTSB<Field<"left", TS<Int>>, Field<"right", TS<Int>>>;
using FixedShape = TSL<FixedBundle, 2>;
constexpr Int cycles = 200000;
Int checksum = 0, evaluations = 0;
struct Pulse {
    static constexpr auto name = "fixed_pulse";
    static constexpr bool schedule_on_start = true;
    static void eval(NodeScheduler scheduler, State<Int> n, Out<TS<Int>> out) {
        out.set(n.get()); n.set(n.get()+1);
        if (n.get()<cycles) scheduler.schedule(MIN_TD);
    }
};
struct OwnedProducer {
    static constexpr auto name = "fixed_owned";
    static void eval(In<"n",TS<Int>> n, Scalar<"offset",Int> offset, Out<FixedShape> out) {
        for (std::size_t a=0;a<2;++a) for (std::size_t b=0;b<2;++b) {
            auto list = out.base().as_list();
            auto parent = list.at(a);
            auto bundle = parent.as_bundle();
            auto child = bundle.at(b);
            Out<TS<Int>>{std::move(child),out.evaluation_time()}.set(n.value()+offset.value()+Int(a*2+b));
        }
    }
};
struct Leaf {
    static constexpr auto name = "fixed_leaf";
    static void eval(In<"n",TS<Int>> n, Scalar<"offset",Int> offset, Out<TS<Int>> out) {out.set(n.value()+offset.value());}
};
struct Route {
    static constexpr auto name = "fixed_route";
    static void eval(In<"n",TS<Int>> n,
        In<"a",FixedShape,InputValidity::Unchecked,InputActivity::Passive> a,
        In<"b",FixedShape,InputValidity::Unchecked,InputActivity::Passive> b,
        Out<REF<FixedShape>> out) {out.set(n.value()%2 ? b.base().reference() : a.base().reference());}
};
struct Sink {
    static constexpr auto name = "fixed_sum";
    static void eval(In<"value",FixedShape> value) {
        for (std::size_t a=0;a<2;++a) for (std::size_t b=0;b<2;++b) {
            auto list = value.base().as_list();
            auto parent = list.at(a);
            auto bundle = parent.as_bundle();
            auto child = bundle.at(b);
            const auto value = child.value();
            checksum += value.as<Int>();
        }
        ++evaluations;
    }
};
template<int Mode> struct Scenario {
    static constexpr auto name = "fixed_scenario";
    static void compose(Wiring &w) {
        auto n=wire<Pulse>(w);
        if constexpr (Mode==1) {
            auto a=stdlib::to_tsb<FixedBundle>(w,wire<Leaf>(w,n,Int{0}),wire<Leaf>(w,n,Int{1}));
            auto b=stdlib::to_tsb<FixedBundle>(w,wire<Leaf>(w,n,Int{2}),wire<Leaf>(w,n,Int{3}));
            wire<Sink>(w,stdlib::to_tsl<FixedShape>(w,a,b).template as<FixedShape>());
        } else {
            auto a=wire<OwnedProducer>(w,n,Int{0});
            if constexpr (Mode==2) wire<Sink>(w,wire<Route>(w,n,a,wire<OwnedProducer>(w,n,Int{100})));
            else wire<Sink>(w,a);
        }
    }
};
template<int Mode> double run() {
    auto graph=build_graph<Scenario<Mode>>();
    GraphExecutorBuilder builder;
    builder.graph_builder(std::move(graph)).start_time(MIN_ST).end_time(MIN_ST+TimeDelta{cycles+8});
    auto executor=builder.make_executor(); auto view=executor.view();
    const auto begin=std::chrono::steady_clock::now(); view.run();
    return std::chrono::duration<double>(std::chrono::steady_clock::now()-begin).count();
}
int main(int argc,char **argv) {
    const std::string mode=argc>1 ? argv[1] : "owned";
    (void)TypeRegistry::instance().register_scalar<Int>("int");
    double seconds;
    if (mode=="owned") seconds=run<0>();
    else if (mode=="assembled") seconds=run<1>();
    else if (mode=="reference") seconds=run<2>();
    else return 2;
    const Int expected=2*cycles*(cycles-1)+6*cycles+(mode=="reference" ? 400*(cycles/2) : 0);
    const bool ok=checksum==expected && evaluations==cycles;
    std::printf("{\"ns_per_cycle\":%.3f,\"checksum\":%lld,\"sink_evals\":%lld,\"ok\":%s}\n",
        seconds*1e9/cycles,static_cast<long long>(checksum),static_cast<long long>(evaluations),ok?"true":"false");
    return ok?0:1;
}
