use marix_protocol::{
    IntentEvent, IntentSignature, InvocationEvent,
    InvocationSignature, PlanEvent, PlanSignature, RelayEvent,
    RelaySignature, StepEvent, StepSignature, TaskEvent,
};

use super::TaskRuntime;

impl TaskRuntime {
    pub fn dispatch(&self, event: TaskEvent) {
        match event {
            TaskEvent::Intent(signature, event) => {
                self.dispatch_intent(signature, event);
            }
            TaskEvent::IntentStart(signature) => {
                self.start_intent(signature);
            }
            TaskEvent::Plan(signature, event) => {
                self.dispatch_plan(signature, event);
            }
            TaskEvent::Step(signature, event) => {
                self.dispatch_step(signature, event);
            }
            TaskEvent::Invocation(signature, event) => {
                self.dispatch_invocation(signature, event);
            }
            TaskEvent::InvocationStart(signature) => {
                self.start_invocation(signature);
            }
            TaskEvent::Relay(signature, event) => {
                self.dispatch_relay(signature, event);
            }
            TaskEvent::Update(signature, status) => {
                self.on_root_update(signature, status);
            }
            TaskEvent::Cancel => self.cancel_task(),
        }
    }
}

// -- Private -- //

impl TaskRuntime {
    fn start_intent(&self, signature: IntentSignature) {
        let Some(intent) = self
            .state
            .intents
            .with(&signature, Clone::clone)
        else {
            self.fail_task(format!(
                "intent {signature} start failed: intent not found",
            ));
            return;
        };
        intent.start();
    }

    fn dispatch_intent(
        &self,
        signature: IntentSignature,
        event: IntentEvent,
    ) {
        let Some(intent) = self
            .state
            .intents
            .with(&signature, Clone::clone)
        else {
            self.fail_task(format!(
                "intent {signature} event {event:?} not dispatched: \
                 intent not found",
            ));
            return;
        };
        intent.dispatch(event);
    }

    fn dispatch_plan(
        &self,
        signature: PlanSignature,
        event: PlanEvent,
    ) {
        let Some(plan) = self.state.plans.with(
            &signature,
            Clone::clone,
        ) else {
            self.fail_task(format!(
                "plan {signature} event {event:?} not dispatched: \
                 plan not found",
            ));
            return;
        };
        plan.dispatch(event);
    }

    fn dispatch_step(
        &self,
        signature: StepSignature,
        event: StepEvent,
    ) {
        let Some(step) = self.state.steps.with(
            &signature,
            Clone::clone,
        ) else {
            self.fail_task(format!(
                "step {signature} event {event:?} not dispatched: \
                 step not found",
            ));
            return;
        };
        step.dispatch(event);
    }

    fn dispatch_invocation(
        &self,
        signature: InvocationSignature,
        event: InvocationEvent,
    ) {
        let Some(invocation) = self.state.invocations.with(
            &signature,
            Clone::clone,
        ) else {
            self.fail_task(format!(
                "invocation {signature} event {event:?} not dispatched: \
                 invocation not found",
            ));
            return;
        };
        invocation.dispatch(event);
    }

    fn start_invocation(&self, signature: InvocationSignature) {
        let Some(invocation) = self
            .state
            .invocations
            .with(&signature, Clone::clone)
        else {
            self.fail_task(format!(
                "invocation {signature} start failed: invocation not found",
            ));
            return;
        };
        invocation.start();
    }

    fn dispatch_relay(
        &self,
        signature: RelaySignature,
        event: RelayEvent,
    ) {
        let Some(relay) = self.state.relays.with(
            &signature,
            Clone::clone,
        ) else {
            self.fail_task(format!(
                "relay {signature} event {event:?} not dispatched: \
                 relay not found",
            ));
            return;
        };
        relay.dispatch(event);
    }
}
