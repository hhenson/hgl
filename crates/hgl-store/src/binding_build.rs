//! Cold allocation of endpoint views and finite-domain subscriptions.
use crate::endpoints::{Endpoints, InputId, Kind, OutputId, Scopes};
use hgl_types::NodeId;
/// Allocate one input and its stable fixed child projections in the current scope.
pub fn input(
    endpoints: &mut Endpoints,
    scopes: &mut Scopes,
    owner: NodeId,
    kind: Kind,
    active: bool,
) -> InputId {
    let id = endpoints.add_input(owner, kind, active, scopes.current);
    scopes.reserve(scopes.current, owner);
    endpoints.inputs[id.0 as usize].scope_position = scopes.record_input(id);
    for position in 0..endpoints.input(id).kind.len() {
        let child = input(
            endpoints,
            scopes,
            owner,
            endpoints.input(id).kind.child(position).clone(),
            active,
        );
        endpoints.inputs[child.0 as usize].parent = Some((
            id,
            i64::try_from(position).unwrap_or_else(|_| unreachable!()),
        ));
        endpoints.inputs[id.0 as usize].fixed.push(child);
    }
    id
}
fn attach(endpoints: &mut Endpoints, input: InputId, output: OutputId) {
    let out = &mut endpoints.outputs[output.0 as usize];
    let position = out.watchers.len();
    out.watchers.push(input);
    let view = &mut endpoints.inputs[input.0 as usize];
    view.source_position = position;
    view.source = Some(output);
    view.slot = out.slot;
}
fn projection(endpoints: &mut Endpoints, scopes: &mut Scopes, id: InputId, source: OutputId) {
    if endpoints.input(id).source.is_none() {
        attach(endpoints, id, source);
    }
    for n in 0..endpoints.input(id).fixed.len() {
        projection(
            endpoints,
            scopes,
            endpoints.input(id).fixed[n],
            endpoints.output(source).fixed[n],
        );
    }
    if !endpoints.output(source).members.live.prepared()
        || endpoints.input(id).members.live.prepared()
    {
        return;
    }
    let view = endpoints.input(id);
    let (owner, active, scope) = (view.owner, view.active, view.scope);
    let previous = std::mem::replace(&mut scopes.current, scope);
    let mut children = Vec::new();
    for n in 0..endpoints.output(source).members.prepared.len() {
        let (key, output) = endpoints.output(source).members.prepared[n];
        let child = input(
            endpoints,
            scopes,
            owner,
            endpoints.output(output).kind.clone(),
            active,
        );
        endpoints.inputs[child.0 as usize].parent = Some((id, key));
        projection(endpoints, scopes, child, output);
        children.push((key, child));
    }
    endpoints.inputs[id.0 as usize].members.prepare(children);
    if endpoints.output(source).members.live.pooled() {
        endpoints.inputs[id.0 as usize].members.pool();
    }
    scopes.current = previous;
}
/// Prepare every bound finite collection view after wiring, before any target starts.
pub fn projections(endpoints: &mut Endpoints, scopes: &mut Scopes) {
    for n in 0..endpoints.inputs.len() {
        let id = InputId(
            u32::try_from(n).unwrap_or_else(|_| unreachable!("endpoint capacity exceeded")),
        );
        if let Some(source) = endpoints.input(id).source {
            projection(endpoints, scopes, id, source);
        }
    }
}

/// Establish an absent finite output domain whose children were allocated cold.
/// # Panics
/// Children must have the root member shape, owner and scope, and no parent.
pub fn collection(endpoints: &mut Endpoints, root: OutputId, children: Vec<(i64, OutputId)>) {
    for &(key, child) in &children {
        let output = endpoints.output(child);
        assert!(
            endpoints.output(root).kind.member() == Some(&output.kind) && output.parent.is_none(),
            "prepared member shape/parent mismatch"
        );
        assert!(
            output.owner == endpoints.output(root).owner
                && output.scope == endpoints.output(root).scope,
            "prepared member ownership mismatch"
        );
        endpoints.outputs[child.0 as usize].parent = Some((root, key));
    }
    endpoints.outputs[root.0 as usize].members.prepare(children);
}

/// Find a retained input child or allocate one on the legacy dynamic path.
pub fn member_input(
    endpoints: &mut Endpoints,
    scopes: &mut Scopes,
    parent: InputId,
    key: i64,
) -> InputId {
    let members = &endpoints.input(parent).members;
    if let Some(&child) = members.live.get(key) {
        return child;
    }
    let child = members
        .prepared_child(key)
        .or_else(|| {
            endpoints.inputs[parent.0 as usize]
                .members
                .removed
                .remove(key)
        })
        .unwrap_or_else(|| {
            let view = endpoints.input(parent);
            let (owner, active, scope) = (view.owner, view.active, view.scope);
            let kind = view
                .kind
                .member()
                .unwrap_or_else(|| unreachable!("membership input"))
                .clone();
            let previous = std::mem::replace(&mut scopes.current, scope);
            let child = input(endpoints, scopes, owner, kind, active);
            scopes.current = previous;
            child
        });
    endpoints.inputs[parent.0 as usize]
        .members
        .insert(key, child);
    child
}
