// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use arm_gic::gicv3::{GicV3, IntId};
use log::{error, trace};
use percore::{exception_free, ExceptionLock};
use smccc::{psci::system_off, Hvc};
use spin::mutex::SpinMutex;

type IrqHandler = &'static (dyn Fn(IntId) + Sync);

static IRQ_HANDLER: ExceptionLock<SpinMutex<Option<IrqHandler>>> =
    ExceptionLock::new(SpinMutex::new(None));

/// Sets the IRQ handler to the given function.
pub fn set_irq_handler(handler: Option<IrqHandler>) {
    exception_free(|token| *IRQ_HANDLER.borrow(token).lock() = handler);
}

#[no_mangle]
extern "C" fn sync_exception_current(_elr: u64, _spsr: u64) {
    error!("sync_exception_current");
    system_off::<Hvc>().unwrap();
}

#[no_mangle]
extern "C" fn irq_current(_elr: u64, _spsr: u64) {
    trace!("irq_current");
    let intid = GicV3::get_and_acknowledge_interrupt().expect("No pending interrupt");
    trace!("IRQ: {:?}", intid);
    exception_free(|token| {
        if let Some(handler) = IRQ_HANDLER.borrow(token).lock().as_ref() {
            handler(intid);
        } else {
            panic!("Unexpected IRQ {:?} with no handler", intid);
        }
    });
}

#[no_mangle]
extern "C" fn fiq_current(_elr: u64, _spsr: u64) {
    error!("fiq_current");
    system_off::<Hvc>().unwrap();
}

#[no_mangle]
extern "C" fn serr_current(_elr: u64, _spsr: u64) {
    error!("serr_current");
    system_off::<Hvc>().unwrap();
}

#[no_mangle]
extern "C" fn sync_lower(_elr: u64, _spsr: u64) {
    error!("sync_lower");
    system_off::<Hvc>().unwrap();
}

#[no_mangle]
extern "C" fn irq_lower(_elr: u64, _spsr: u64) {
    error!("irq_lower");
    system_off::<Hvc>().unwrap();
}

#[no_mangle]
extern "C" fn fiq_lower(_elr: u64, _spsr: u64) {
    error!("fiq_lower");
    system_off::<Hvc>().unwrap();
}

#[no_mangle]
extern "C" fn serr_lower(_elr: u64, _spsr: u64) {
    error!("serr_lower");
    system_off::<Hvc>().unwrap();
}
