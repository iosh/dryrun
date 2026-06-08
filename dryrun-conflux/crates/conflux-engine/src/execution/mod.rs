use cfx_execute_helper::estimation::EstimationContext;
use cfx_executor::{machine::Machine, state::State};
use cfx_vm_types::{Env, Spec};

pub struct ExecutionContextParts<'a> {
    pub state: &'a mut State,
    pub env: &'a Env,
    pub machine: &'a Machine,
    pub spec: &'a Spec,
}

pub fn probe_estimation_context(parts: ExecutionContextParts<'_>) {
    let _context = EstimationContext::new(parts.state, parts.env, parts.machine, parts.spec);
}
