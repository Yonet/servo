/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#![allow(unsafe_code)]

use crate::dom::bindings::cell::DomRefCell;
use crate::dom::bindings::codegen::Bindings::GPUAdapterBinding::GPULimits;
use crate::dom::bindings::codegen::Bindings::GPUBindGroupBinding::GPUBindGroupDescriptor;
use crate::dom::bindings::codegen::Bindings::GPUBindGroupLayoutBinding::{
    GPUBindGroupLayoutDescriptor, GPUBindGroupLayoutEntry, GPUBindingType,
};
use crate::dom::bindings::codegen::Bindings::GPUBufferBinding::GPUBufferDescriptor;
use crate::dom::bindings::codegen::Bindings::GPUComputePipelineBinding::GPUComputePipelineDescriptor;
use crate::dom::bindings::codegen::Bindings::GPUDeviceBinding::{
    GPUCommandEncoderDescriptor, GPUDeviceMethods,
};
use crate::dom::bindings::codegen::Bindings::GPUPipelineLayoutBinding::GPUPipelineLayoutDescriptor;
use crate::dom::bindings::codegen::Bindings::GPUSamplerBinding::{
    GPUAddressMode, GPUCompareFunction, GPUFilterMode, GPUSamplerDescriptor,
};
use crate::dom::bindings::codegen::Bindings::GPUShaderModuleBinding::GPUShaderModuleDescriptor;
use crate::dom::bindings::codegen::UnionTypes::Uint32ArrayOrString::{String, Uint32Array};
use crate::dom::bindings::reflector::{reflect_dom_object, DomObject};
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::bindings::str::DOMString;
use crate::dom::bindings::trace::RootedTraceableBox;
use crate::dom::eventtarget::EventTarget;
use crate::dom::globalscope::GlobalScope;
use crate::dom::gpuadapter::GPUAdapter;
use crate::dom::gpubindgroup::GPUBindGroup;
use crate::dom::gpubindgrouplayout::GPUBindGroupLayout;
use crate::dom::gpubuffer::{GPUBuffer, GPUBufferState};
use crate::dom::gpucommandencoder::GPUCommandEncoder;
use crate::dom::gpucomputepipeline::GPUComputePipeline;
use crate::dom::gpupipelinelayout::GPUPipelineLayout;
use crate::dom::gpuqueue::GPUQueue;
use crate::dom::gpusampler::GPUSampler;
use crate::dom::gpushadermodule::GPUShaderModule;
use crate::script_runtime::JSContext as SafeJSContext;
use dom_struct::dom_struct;
use ipc_channel::ipc;
use js::jsapi::{Heap, JSObject};
use js::jsval::{JSVal, ObjectValue};
use js::typedarray::{ArrayBuffer, CreateWith};
use std::collections::{HashMap, HashSet};
use std::ptr::{self, NonNull};
use webgpu::wgpu::binding_model::{
    BindGroupEntry, BindGroupLayoutEntry, BindingResource, BindingType, BufferBinding,
};
use webgpu::{wgt, WebGPU, WebGPUDevice, WebGPUQueue, WebGPURequest, WebGPUSampler};

#[dom_struct]
pub struct GPUDevice {
    eventtarget: EventTarget,
    #[ignore_malloc_size_of = "channels are hard"]
    channel: WebGPU,
    adapter: Dom<GPUAdapter>,
    #[ignore_malloc_size_of = "mozjs"]
    extensions: Heap<*mut JSObject>,
    #[ignore_malloc_size_of = "mozjs"]
    limits: Heap<*mut JSObject>,
    label: DomRefCell<Option<DOMString>>,
    device: WebGPUDevice,
    default_queue: Dom<GPUQueue>,
}

impl GPUDevice {
    fn new_inherited(
        channel: WebGPU,
        adapter: &GPUAdapter,
        extensions: Heap<*mut JSObject>,
        limits: Heap<*mut JSObject>,
        device: WebGPUDevice,
        queue: &GPUQueue,
    ) -> GPUDevice {
        Self {
            eventtarget: EventTarget::new_inherited(),
            channel,
            adapter: Dom::from_ref(adapter),
            extensions,
            limits,
            label: DomRefCell::new(None),
            device,
            default_queue: Dom::from_ref(queue),
        }
    }

    #[allow(unsafe_code)]
    pub fn new(
        global: &GlobalScope,
        channel: WebGPU,
        adapter: &GPUAdapter,
        extensions: Heap<*mut JSObject>,
        limits: Heap<*mut JSObject>,
        device: WebGPUDevice,
        queue: WebGPUQueue,
    ) -> DomRoot<GPUDevice> {
        let queue = GPUQueue::new(global, channel.clone(), queue);
        reflect_dom_object(
            Box::new(GPUDevice::new_inherited(
                channel, adapter, extensions, limits, device, &queue,
            )),
            global,
        )
    }
}

impl GPUDevice {
    fn validate_buffer_descriptor(
        &self,
        descriptor: &GPUBufferDescriptor,
    ) -> (bool, wgt::BufferDescriptor<std::string::String>) {
        // TODO: Record a validation error in the current scope if the descriptor is invalid.
        let wgpu_usage = wgt::BufferUsage::from_bits(descriptor.usage);
        let valid = wgpu_usage.is_some() && descriptor.size > 0;

        if valid {
            (
                true,
                wgt::BufferDescriptor {
                    size: descriptor.size,
                    usage: wgpu_usage.unwrap(),
                    label: Default::default(),
                },
            )
        } else {
            (
                false,
                wgt::BufferDescriptor {
                    size: 0,
                    usage: wgt::BufferUsage::empty(),
                    label: Default::default(),
                },
            )
        }
    }
}

impl GPUDeviceMethods for GPUDevice {
    /// https://gpuweb.github.io/gpuweb/#dom-gpudevice-adapter
    fn Adapter(&self) -> DomRoot<GPUAdapter> {
        DomRoot::from_ref(&self.adapter)
    }

    /// https://gpuweb.github.io/gpuweb/#dom-gpudevice-extensions
    fn Extensions(&self, _cx: SafeJSContext) -> NonNull<JSObject> {
        NonNull::new(self.extensions.get()).unwrap()
    }

    /// https://gpuweb.github.io/gpuweb/#dom-gpudevice-limits
    fn Limits(&self, _cx: SafeJSContext) -> NonNull<JSObject> {
        NonNull::new(self.extensions.get()).unwrap()
    }

    /// https://gpuweb.github.io/gpuweb/#dom-gpudevice-defaultqueue
    fn DefaultQueue(&self) -> DomRoot<GPUQueue> {
        DomRoot::from_ref(&self.default_queue)
    }

    /// https://gpuweb.github.io/gpuweb/#dom-gpuobjectbase-label
    fn GetLabel(&self) -> Option<DOMString> {
        self.label.borrow().clone()
    }

    /// https://gpuweb.github.io/gpuweb/#dom-gpuobjectbase-label
    fn SetLabel(&self, value: Option<DOMString>) {
        *self.label.borrow_mut() = value;
    }

    /// https://gpuweb.github.io/gpuweb/#dom-gpudevice-createbuffer
    fn CreateBuffer(&self, descriptor: &GPUBufferDescriptor) -> DomRoot<GPUBuffer> {
        let (valid, wgpu_descriptor) = self.validate_buffer_descriptor(descriptor);
        let (sender, receiver) = ipc::channel().unwrap();
        let id = self
            .global()
            .wgpu_id_hub()
            .lock()
            .create_buffer_id(self.device.0.backend());
        self.channel
            .0
            .send(WebGPURequest::CreateBuffer {
                sender,
                device_id: self.device.0,
                buffer_id: id,
                descriptor: wgpu_descriptor,
            })
            .expect("Failed to create WebGPU buffer");

        let buffer = receiver.recv().unwrap();

        GPUBuffer::new(
            &self.global(),
            self.channel.clone(),
            buffer,
            self.device,
            GPUBufferState::Unmapped,
            descriptor.size,
            descriptor.usage,
            valid,
            RootedTraceableBox::new(Heap::default()),
        )
    }

    /// https://gpuweb.github.io/gpuweb/#dom-gpudevice-createbuffermapped
    fn CreateBufferMapped(
        &self,
        cx: SafeJSContext,
        descriptor: &GPUBufferDescriptor,
    ) -> Vec<JSVal> {
        let (valid, wgpu_descriptor) = self.validate_buffer_descriptor(descriptor);
        let (sender, receiver) = ipc::channel().unwrap();
        let buffer_id = self
            .global()
            .wgpu_id_hub()
            .lock()
            .create_buffer_id(self.device.0.backend());
        self.channel
            .0
            .send(WebGPURequest::CreateBufferMapped {
                sender,
                device_id: self.device.0,
                buffer_id,
                descriptor: wgpu_descriptor.clone(),
            })
            .expect("Failed to create WebGPU buffer");

        rooted!(in(*cx) let mut js_array_buffer = ptr::null_mut::<JSObject>());
        unsafe {
            assert!(ArrayBuffer::create(
                *cx,
                CreateWith::Length(descriptor.size as u32),
                js_array_buffer.handle_mut(),
            )
            .is_ok());
        }

        let buffer = receiver.recv().unwrap();
        let buff = GPUBuffer::new(
            &self.global(),
            self.channel.clone(),
            buffer,
            self.device,
            GPUBufferState::MappedForWriting,
            wgpu_descriptor.size,
            wgpu_descriptor.usage.bits(),
            valid,
            RootedTraceableBox::from_box(Heap::boxed(js_array_buffer.get())),
        );

        vec![
            ObjectValue(buff.reflector().get_jsobject().get()),
            ObjectValue(js_array_buffer.get()),
        ]
    }

    /// https://gpuweb.github.io/gpuweb/#GPUDevice-createBindGroupLayout
    #[allow(non_snake_case)]
    fn CreateBindGroupLayout(
        &self,
        descriptor: &GPUBindGroupLayoutDescriptor,
    ) -> DomRoot<GPUBindGroupLayout> {
        #[derive(Clone)]
        struct MaxLimits {
            max_uniform_buffers_per_shader_stage: i32,
            max_storage_buffers_per_shader_stage: i32,
            max_sampled_textures_per_shader_stage: i32,
            max_storage_textures_per_shader_stage: i32,
            max_samplers_per_shader_stage: i32,
        }
        let mut storeBindings = HashSet::new();
        // TODO: We should have these limits on device creation
        let limits = GPULimits::empty();

        let mut validation_map = HashMap::new();
        let maxLimits = MaxLimits {
            max_uniform_buffers_per_shader_stage: limits.maxUniformBuffersPerShaderStage as i32,
            max_storage_buffers_per_shader_stage: limits.maxStorageBuffersPerShaderStage as i32,
            max_sampled_textures_per_shader_stage: limits.maxSampledTexturesPerShaderStage as i32,
            max_storage_textures_per_shader_stage: limits.maxStorageTexturesPerShaderStage as i32,
            max_samplers_per_shader_stage: limits.maxSamplersPerShaderStage as i32,
        };
        validation_map.insert(wgt::ShaderStage::VERTEX, maxLimits.clone());
        validation_map.insert(wgt::ShaderStage::FRAGMENT, maxLimits.clone());
        validation_map.insert(wgt::ShaderStage::COMPUTE, maxLimits.clone());
        let mut max_dynamic_uniform_buffers_per_pipeline_layout =
            limits.maxDynamicUniformBuffersPerPipelineLayout as i32;
        let mut max_dynamic_storage_buffers_per_pipeline_layout =
            limits.maxDynamicStorageBuffersPerPipelineLayout as i32;
        let mut valid = true;

        let bindings = descriptor
            .entries
            .iter()
            .map(|bind| {
                // TODO: binding must be >= 0
                storeBindings.insert(bind.binding);
                let visibility = match wgt::ShaderStage::from_bits(bind.visibility) {
                    Some(visibility) => visibility,
                    None => {
                        valid = false;
                        wgt::ShaderStage::from_bits(0).unwrap()
                    },
                };
                let ty = match bind.type_ {
                    GPUBindingType::Uniform_buffer => {
                        if let Some(limit) = validation_map.get_mut(&visibility) {
                            limit.max_uniform_buffers_per_shader_stage -= 1;
                        }
                        if bind.hasDynamicOffset {
                            max_dynamic_uniform_buffers_per_pipeline_layout -= 1;
                        };
                        BindingType::UniformBuffer
                    },
                    GPUBindingType::Storage_buffer => {
                        if let Some(limit) = validation_map.get_mut(&visibility) {
                            limit.max_storage_buffers_per_shader_stage -= 1;
                        }
                        if bind.hasDynamicOffset {
                            max_dynamic_storage_buffers_per_pipeline_layout -= 1;
                        };
                        BindingType::StorageBuffer
                    },
                    GPUBindingType::Readonly_storage_buffer => {
                        if let Some(limit) = validation_map.get_mut(&visibility) {
                            limit.max_storage_buffers_per_shader_stage -= 1;
                        }
                        if bind.hasDynamicOffset {
                            max_dynamic_storage_buffers_per_pipeline_layout -= 1;
                        };
                        BindingType::ReadonlyStorageBuffer
                    },
                    GPUBindingType::Sampled_texture => {
                        if let Some(limit) = validation_map.get_mut(&visibility) {
                            limit.max_sampled_textures_per_shader_stage -= 1;
                        }
                        if bind.hasDynamicOffset {
                            valid = false
                        };
                        BindingType::SampledTexture
                    },
                    GPUBindingType::Readonly_storage_texture => {
                        if let Some(limit) = validation_map.get_mut(&visibility) {
                            limit.max_storage_textures_per_shader_stage -= 1;
                        }
                        if bind.hasDynamicOffset {
                            valid = false
                        };
                        BindingType::ReadonlyStorageTexture
                    },
                    GPUBindingType::Writeonly_storage_texture => {
                        if let Some(limit) = validation_map.get_mut(&visibility) {
                            limit.max_storage_textures_per_shader_stage -= 1;
                        }
                        if bind.hasDynamicOffset {
                            valid = false
                        };
                        BindingType::WriteonlyStorageTexture
                    },
                    GPUBindingType::Sampler => {
                        if let Some(limit) = validation_map.get_mut(&visibility) {
                            limit.max_samplers_per_shader_stage -= 1;
                        }
                        if bind.hasDynamicOffset {
                            valid = false
                        };
                        BindingType::Sampler
                    },
                };

                BindGroupLayoutEntry {
                    binding: bind.binding,
                    visibility,
                    ty,
                    has_dynamic_offset: bind.hasDynamicOffset,
                    multisampled: bind.multisampled,
                    // Use as default for now
                    texture_component_type: wgt::TextureComponentType::Float,
                    storage_texture_format: wgt::TextureFormat::Rgba8UnormSrgb,
                    view_dimension: wgt::TextureViewDimension::D2,
                }
            })
            .collect::<Vec<BindGroupLayoutEntry>>();

        // bindings are unique
        valid &= storeBindings.len() == bindings.len();

        // Ensure that values do not exceed the max limit for each ShaderStage.
        valid &= validation_map.values().all(|stage| {
            stage.max_uniform_buffers_per_shader_stage >= 0 &&
                stage.max_storage_buffers_per_shader_stage >= 0 &&
                stage.max_sampled_textures_per_shader_stage >= 0 &&
                stage.max_storage_textures_per_shader_stage >= 0 &&
                stage.max_samplers_per_shader_stage >= 0
        });

        // DynamicValues does not exceed the max limit for the pipeline
        valid &= max_dynamic_uniform_buffers_per_pipeline_layout >= 0 &&
            max_dynamic_storage_buffers_per_pipeline_layout >= 0;

        let (sender, receiver) = ipc::channel().unwrap();
        let bind_group_layout_id = self
            .global()
            .wgpu_id_hub()
            .lock()
            .create_bind_group_layout_id(self.device.0.backend());
        self.channel
            .0
            .send(WebGPURequest::CreateBindGroupLayout {
                sender,
                device_id: self.device.0,
                bind_group_layout_id,
                bindings: bindings.clone(),
            })
            .expect("Failed to create WebGPU BindGroupLayout");

        let bgl = receiver.recv().unwrap();

        let binds = descriptor
            .entries
            .iter()
            .map(|bind| GPUBindGroupLayoutEntry {
                binding: bind.binding,
                hasDynamicOffset: bind.hasDynamicOffset,
                multisampled: bind.multisampled,
                type_: bind.type_,
                visibility: bind.visibility,
                //texture_dimension: bind.texture_dimension
            })
            .collect::<Vec<_>>();

        GPUBindGroupLayout::new(&self.global(), self.channel.clone(), bgl, binds, valid)
    }

    /// https://gpuweb.github.io/gpuweb/#dom-gpudevice-createpipelinelayout
    fn CreatePipelineLayout(
        &self,
        descriptor: &GPUPipelineLayoutDescriptor,
    ) -> DomRoot<GPUPipelineLayout> {
        // TODO: We should have these limits on device creation
        let limits = GPULimits::empty();
        let mut bind_group_layouts = Vec::new();
        let mut bgl_ids = Vec::new();
        let mut max_dynamic_uniform_buffers_per_pipeline_layout =
            limits.maxDynamicUniformBuffersPerPipelineLayout as i32;
        let mut max_dynamic_storage_buffers_per_pipeline_layout =
            limits.maxDynamicStorageBuffersPerPipelineLayout as i32;
        descriptor.bindGroupLayouts.iter().for_each(|each| {
            if each.is_valid() {
                let id = each.id();
                bind_group_layouts.push(id);
                bgl_ids.push(id.0);
            }
            each.bindings().iter().for_each(|bind| {
                match bind.type_ {
                    GPUBindingType::Uniform_buffer => {
                        if bind.hasDynamicOffset {
                            max_dynamic_uniform_buffers_per_pipeline_layout -= 1;
                        };
                    },
                    GPUBindingType::Storage_buffer => {
                        if bind.hasDynamicOffset {
                            max_dynamic_storage_buffers_per_pipeline_layout -= 1;
                        };
                    },
                    GPUBindingType::Readonly_storage_buffer => {
                        if bind.hasDynamicOffset {
                            max_dynamic_storage_buffers_per_pipeline_layout -= 1;
                        };
                    },
                    _ => {},
                };
            });
        });

        let valid = descriptor.bindGroupLayouts.len() <= limits.maxBindGroups as usize &&
            descriptor.bindGroupLayouts.len() == bind_group_layouts.len() &&
            max_dynamic_uniform_buffers_per_pipeline_layout >= 0 &&
            max_dynamic_storage_buffers_per_pipeline_layout >= 0;

        let (sender, receiver) = ipc::channel().unwrap();
        let pipeline_layout_id = self
            .global()
            .wgpu_id_hub()
            .lock()
            .create_pipeline_layout_id(self.device.0.backend());
        self.channel
            .0
            .send(WebGPURequest::CreatePipelineLayout {
                sender,
                device_id: self.device.0,
                pipeline_layout_id,
                bind_group_layouts: bgl_ids,
            })
            .expect("Failed to create WebGPU PipelineLayout");

        let pipeline_layout = receiver.recv().unwrap();
        GPUPipelineLayout::new(&self.global(), bind_group_layouts, pipeline_layout, valid)
    }

    /// https://gpuweb.github.io/gpuweb/#dom-gpudevice-createbindgroup
    fn CreateBindGroup(&self, descriptor: &GPUBindGroupDescriptor) -> DomRoot<GPUBindGroup> {
        let alignment: u64 = 256;
        let mut valid = descriptor.layout.bindings().len() == descriptor.entries.len();

        valid &= descriptor.entries.iter().all(|bind| {
            let buffer_size = bind.resource.buffer.size();
            let resource_size = bind.resource.size.unwrap_or(buffer_size);
            let length = bind.resource.offset.checked_add(resource_size);
            let usage = wgt::BufferUsage::from_bits(bind.resource.buffer.usage()).unwrap();

            length.is_some() &&
            buffer_size >= length.unwrap() && // check buffer OOB
            bind.resource.offset % alignment == 0 && // check alignment
            bind.resource.offset < buffer_size && // on Vulkan offset must be less than size of buffer
            descriptor.layout.bindings().iter().any(|layout_bind| {
                let ty = match layout_bind.type_ {
                    GPUBindingType::Storage_buffer  => wgt::BufferUsage::STORAGE,
                    // GPUBindingType::Readonly_storage_buffer  => BufferUsage::STORAGE_READ,
                    GPUBindingType::Uniform_buffer => wgt::BufferUsage::UNIFORM,
                    _ => unimplemented!(),
                };
                // binding must be present in layout
                layout_bind.binding == bind.binding &&
                // binding must contain one buffer of its type
                usage.contains(ty)
            })
        });

        let bindings = descriptor
            .entries
            .iter()
            .map(|bind| BindGroupEntry {
                binding: bind.binding,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: bind.resource.buffer.id().0,
                    offset: bind.resource.offset,
                    size: bind.resource.size.unwrap_or(bind.resource.buffer.size()),
                }),
            })
            .collect::<Vec<_>>();
        let (sender, receiver) = ipc::channel().unwrap();
        let bind_group_id = self
            .global()
            .wgpu_id_hub()
            .lock()
            .create_bind_group_id(self.device.0.backend());
        self.channel
            .0
            .send(WebGPURequest::CreateBindGroup {
                sender,
                device_id: self.device.0,
                bind_group_id,
                bind_group_layout_id: descriptor.layout.id().0,
                bindings,
            })
            .expect("Failed to create WebGPU BindGroup");

        let bind_group = receiver.recv().unwrap();
        GPUBindGroup::new(&self.global(), bind_group, valid)
    }

    /// https://gpuweb.github.io/gpuweb/#dom-gpudevice-createshadermodule
    fn CreateShaderModule(
        &self,
        descriptor: RootedTraceableBox<GPUShaderModuleDescriptor>,
    ) -> DomRoot<GPUShaderModule> {
        let (sender, receiver) = ipc::channel().unwrap();
        let program: Vec<u32> = match &descriptor.code {
            Uint32Array(program) => program.to_vec(),
            String(program) => program.chars().map(|c| c as u32).collect::<Vec<u32>>(),
        };
        let program_id = self
            .global()
            .wgpu_id_hub()
            .lock()
            .create_shader_module_id(self.device.0.backend());
        self.channel
            .0
            .send(WebGPURequest::CreateShaderModule {
                sender,
                device_id: self.device.0,
                program_id,
                program,
            })
            .expect("Failed to create WebGPU ShaderModule");

        let shader_module = receiver.recv().unwrap();
        GPUShaderModule::new(&self.global(), shader_module)
    }

    /// https://gpuweb.github.io/gpuweb/#dom-gpudevice-createcomputepipeline
    fn CreateComputePipeline(
        &self,
        descriptor: &GPUComputePipelineDescriptor,
    ) -> DomRoot<GPUComputePipeline> {
        let pipeline = descriptor.parent.layout.id();
        let program = descriptor.computeStage.module.id();
        let entry_point = descriptor.computeStage.entryPoint.to_string();
        let compute_pipeline_id = self
            .global()
            .wgpu_id_hub()
            .lock()
            .create_compute_pipeline_id(self.device.0.backend());
        let (sender, receiver) = ipc::channel().unwrap();
        self.channel
            .0
            .send(WebGPURequest::CreateComputePipeline {
                sender,
                device_id: self.device.0,
                compute_pipeline_id,
                pipeline_layout_id: pipeline.0,
                program_id: program.0,
                entry_point,
            })
            .expect("Failed to create WebGPU ComputePipeline");

        let compute_pipeline = receiver.recv().unwrap();
        GPUComputePipeline::new(&self.global(), compute_pipeline)
    }
    /// https://gpuweb.github.io/gpuweb/#dom-gpudevice-createcommandencoder
    fn CreateCommandEncoder(
        &self,
        _descriptor: &GPUCommandEncoderDescriptor,
    ) -> DomRoot<GPUCommandEncoder> {
        let (sender, receiver) = ipc::channel().unwrap();
        let command_encoder_id = self
            .global()
            .wgpu_id_hub()
            .lock()
            .create_command_encoder_id(self.device.0.backend());
        self.channel
            .0
            .send(WebGPURequest::CreateCommandEncoder {
                sender,
                device_id: self.device.0,
                command_encoder_id,
            })
            .expect("Failed to create WebGPU command encoder");
        let encoder = receiver.recv().unwrap();

        GPUCommandEncoder::new(&self.global(), self.channel.clone(), encoder, true)
    }
    /// https://gpuweb.github.io/gpuweb/#dom-gpudevice-createsampler
    fn CreateSampler(&self, descriptor: &GPUSamplerDescriptor) -> DomRoot<GPUSampler> {
        let sampler_id = self
            .global()
            .wgpu_id_hub()
            .lock()
            .create_sampler_id(self.device.0.backend());
        let compare_enable = descriptor.compare.is_some();
        let desc = wgt::SamplerDescriptor {
            label: Default::default(),
            address_mode_u: assign_address_mode(descriptor.addressModeU),
            address_mode_v: assign_address_mode(descriptor.addressModeV),
            address_mode_w: assign_address_mode(descriptor.addressModeW),
            mag_filter: assign_filter_mode(descriptor.magFilter),
            min_filter: assign_filter_mode(descriptor.minFilter),
            mipmap_filter: assign_filter_mode(descriptor.mipmapFilter),
            lod_min_clamp: *descriptor.lodMinClamp,
            lod_max_clamp: *descriptor.lodMaxClamp,
            compare: if let Some(c) = descriptor.compare {
                match c {
                    GPUCompareFunction::Never => wgt::CompareFunction::Never,
                    GPUCompareFunction::Less => wgt::CompareFunction::Less,
                    GPUCompareFunction::Equal => wgt::CompareFunction::Equal,
                    GPUCompareFunction::Less_equal => wgt::CompareFunction::LessEqual,
                    GPUCompareFunction::Greater => wgt::CompareFunction::Greater,
                    GPUCompareFunction::Not_equal => wgt::CompareFunction::NotEqual,
                    GPUCompareFunction::Greater_equal => wgt::CompareFunction::GreaterEqual,
                    GPUCompareFunction::Always => wgt::CompareFunction::Always,
                }
            } else {
                wgt::CompareFunction::Undefined
            },
        };
        self.channel
            .0
            .send(WebGPURequest::CreateSampler {
                device_id: self.device.0,
                sampler_id,
                descriptor: desc,
            })
            .expect("Failed to create WebGPU sampler");

        let sampler = WebGPUSampler(sampler_id);

        GPUSampler::new(
            &self.global(),
            self.channel.clone(),
            self.device,
            compare_enable,
            sampler,
            true,
        )
    }
}

fn assign_address_mode(address_mode: GPUAddressMode) -> wgt::AddressMode {
    match address_mode {
        GPUAddressMode::Clamp_to_edge => wgt::AddressMode::ClampToEdge,
        GPUAddressMode::Repeat => wgt::AddressMode::Repeat,
        GPUAddressMode::Mirror_repeat => wgt::AddressMode::MirrorRepeat,
    }
}

fn assign_filter_mode(filter_mode: GPUFilterMode) -> wgt::FilterMode {
    match filter_mode {
        GPUFilterMode::Nearest => wgt::FilterMode::Nearest,
        GPUFilterMode::Linear => wgt::FilterMode::Linear,
    }
}
