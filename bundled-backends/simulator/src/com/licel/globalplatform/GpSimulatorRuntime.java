package com.licel.globalplatform;

import com.licel.jcardsim.base.SimulatorRuntime;

/**
 * Minimal compatibility runtime for jcardsim's optional GlobalPlatform probe.
 *
 * <p>JCIM does not depend on jcardsim's separate GlobalPlatform runtime package for the maintained
 * simulator path, but recent jcardsim builds still attempt to reflectively load this class during
 * startup. Providing a trivial subclass keeps startup quiet and lets the simulator fall back to
 * the default runtime behavior without polluting stderr.</p>
 */
public final class GpSimulatorRuntime extends SimulatorRuntime {}
