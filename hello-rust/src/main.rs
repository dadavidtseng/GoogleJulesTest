use image::GenericImageView;
use windows::{
    core::*,
    Win32::Foundation::*,
};

#[repr(C)]
struct Vertex {
    pos: [f32; 3],
    tex: [f32; 2],
}
    Win32::Graphics::Direct3D::*,
    Win32::Graphics::Direct3D11::*,
    Win32::Graphics::Dxgi::Common::*,
    Win32::Graphics::Dxgi::*,
    Win32::System::LibraryLoader::GetModuleHandleW,
    Win32::UI::WindowsAndMessaging::*,
};

struct D3D11State {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    swap_chain: IDXGISwapChain,
    render_target_view: ID3D11RenderTargetView,
    texture: ID3D11Texture2D,
    vertex_shader: ID3D11VertexShader,
    pixel_shader: ID3D11PixelShader,
    input_layout: ID3D11InputLayout,
    vertex_buffer: ID3D11Buffer,
    index_buffer: ID3D11Buffer,
    sampler_state: ID3D11SamplerState,
}

fn main() -> Result<()> {
    let img = image::open("placeholder.png").unwrap();
    println!("Image dimensions: {}x{}", img.width(), img.height());

    unsafe {
        let instance = GetModuleHandleW(None)?;
        let window_class_name = w!("SampleWindowClass");

        let wc = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hInstance: instance.into(),
            lpszClassName: window_class_name,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            ..Default::default()
        };

        let atom = RegisterClassW(&wc);
        if atom == 0 {
            return Err(Error::from_win32());
        }

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            window_class_name,
            w!("Hello, DirectX!"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            800,
            600,
            None,
            None,
            instance,
            None,
        );

        if hwnd.0 == 0 {
            return Err(Error::from_win32());
        }

        let d3d_state = init_d3d(hwnd, &img)?;

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            TranslateMessage(&message);
            DispatchMessageW(&message);

            let clear_color: [f32; 4] = [0.2, 0.3, 0.4, 1.0];
            d3d_state.context.ClearRenderTargetView(&d3d_state.render_target_view, &clear_color);

            let stride = std::mem::size_of::<Vertex>() as u32;
            let offset = 0;
            d3d_state.context.IASetVertexBuffers(0, 1, Some(&[Some(d3d_state.vertex_buffer.clone())]), Some(&stride), Some(&offset));
            d3d_state.context.IASetIndexBuffer(&d3d_state.index_buffer, DXGI_FORMAT_R32_UINT, 0);
            d3d_state.context.IASetInputLayout(&d3d_state.input_layout);
            d3d_state.context.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);

            d3d_state.context.VSSetShader(&d3d_state.vertex_shader, None);
            d3d_state.context.PSSetShader(&d3d_state.pixel_shader, None);

            let shader_resource_view = d3d_state.device.CreateShaderResourceView(&d3d_state.texture, None)?;
            d3d_state.context.PSSetShaderResources(0, Some(&[Some(shader_resource_view)]));
            d3d_state.context.PSSetSamplers(0, Some(&[Some(d3d_state.sampler_state.clone())]));

            d3d_state.context.DrawIndexed(6, 0, 0);

            d3d_state.swap_chain.Present(1, 0)?;
        }

        Ok(())
    }
}

use image::DynamicImage;

fn init_d3d(hwnd: HWND, img: &DynamicImage) -> Result<D3D11State> {
    unsafe {
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;

        let dxgi_factory: IDXGIFactory = CreateDXGIFactory()?;

        let adapter = dxgi_factory.EnumAdapters(0)?;

        let mut flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT;
        if cfg!(debug_assertions) {
            flags |= D3D11_CREATE_DEVICE_DEBUG;
        }

        D3D11CreateDevice(
            &adapter,
            D3D_DRIVER_TYPE_UNKNOWN,
            None,
            flags,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )?;

        let device = device.unwrap();
        let context = context.unwrap();

        let mut rect = RECT::default();
        GetClientRect(hwnd, &mut rect)?;

        let swap_chain_desc = DXGI_SWAP_CHAIN_DESC {
            BufferDesc: DXGI_MODE_DESC {
                Width: (rect.right - rect.left) as u32,
                Height: (rect.bottom - rect.top) as u32,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                ..Default::default()
            },
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 1,
            OutputWindow: hwnd,
            Windowed: BOOL(1),
            ..Default::default()
        };

        let swap_chain = dxgi_factory.CreateSwapChain(&device, &swap_chain_desc)?;

        let back_buffer: ID3D11Texture2D = swap_chain.GetBuffer(0)?;

        let render_target_view = device.CreateRenderTargetView(&back_buffer, None)?;

        let texture_desc = D3D11_TEXTURE2D_DESC {
            Width: img.width(),
            Height: img.height(),
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE,
            ..Default::default()
        };

        let rgba_image = img.to_rgba8();
        let subresource_data = D3D11_SUBRESOURCE_DATA {
            pSysMem: rgba_image.as_raw().as_ptr() as *const _,
            SysMemPitch: img.width() * 4,
            ..Default::default()
        };

        let mut texture: Option<ID3D11Texture2D> = None;
        device.CreateTexture2D(&texture_desc, Some(&subresource_data), Some(&mut texture))?;
        let texture = texture.unwrap();

        context.OMSetRenderTargets(Some(&[Some(render_target_view.clone())]), None);

        let shader_code = "
            struct VS_INPUT {
                float4 pos : POSITION;
                float2 tex : TEXCOORD0;
            };

            struct PS_INPUT {
                float4 pos : SV_POSITION;
                float2 tex : TEXCOORD0;
            };

            PS_INPUT VS(VS_INPUT input) {
                PS_INPUT output = (PS_INPUT)0;
                output.pos = input.pos;
                output.tex = input.tex;
                return output;
            }

            Texture2D tx : register(t0);
            SamplerState sm : register(s0);

            float4 PS(PS_INPUT input) : SV_Target {
                return tx.Sample(sm, input.tex);
            }
        ";

        let mut vertex_shader_blob: Option<ID3DBlob> = None;
        let mut error_blob: Option<ID3DBlob> = None;
        D3DCompile(
            shader_code.as_ptr() as *const _,
            shader_code.len(),
            None,
            None,
            None,
            s!("VS"),
            s!("vs_5_0"),
            0,
            0,
            &mut vertex_shader_blob,
            Some(&mut error_blob),
        )?;

        let vertex_shader = device.CreateVertexShader(vertex_shader_blob.as_ref().unwrap().GetBufferPointer(), vertex_shader_blob.as_ref().unwrap().GetBufferSize(), None)?;

        let mut pixel_shader_blob: Option<ID3DBlob> = None;
        D3DCompile(
            shader_code.as_ptr() as *const _,
            shader_code.len(),
            None,
            None,
            None,
            s!("PS"),
            s!("ps_5_0"),
            0,
            0,
            &mut pixel_shader_blob,
            Some(&mut error_blob),
        )?;

        let pixel_shader = device.CreatePixelShader(pixel_shader_blob.as_ref().unwrap().GetBufferPointer(), pixel_shader_blob.as_ref().unwrap().GetBufferSize(), None)?;


        let vertices = [
            Vertex { pos: [-0.5, -0.5, 0.0], tex: [0.0, 1.0] },
            Vertex { pos: [-0.5, 0.5, 0.0], tex: [0.0, 0.0] },
            Vertex { pos: [0.5, 0.5, 0.0], tex: [1.0, 0.0] },
            Vertex { pos: [0.5, -0.5, 0.0], tex: [1.0, 1.0] },
        ];

        let buffer_desc = D3D11_BUFFER_DESC {
            ByteWidth: std::mem::size_of_val(&vertices) as u32,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_VERTEX_BUFFER,
            ..Default::default()
        };

        let subresource_data = D3D11_SUBRESOURCE_DATA {
            pSysMem: vertices.as_ptr() as *const _,
            ..Default::default()
        };

        let vertex_buffer = device.CreateBuffer(&buffer_desc, Some(&subresource_data))?;

        let input_element_desc = [
            D3D11_INPUT_ELEMENT_DESC {
                SemanticName: s!("POSITION"),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32B32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 0,
                InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
            D3D11_INPUT_ELEMENT_DESC {
                SemanticName: s!("TEXCOORD"),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 12,
                InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
        ];

        let input_layout = device.CreateInputLayout(&input_element_desc, vertex_shader_blob.as_ref().unwrap().GetBufferPointer(), vertex_shader_blob.as_ref().unwrap().GetBufferSize())?;

        let sampler_desc = D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            AddressU: D3D11_TEXTURE_ADDRESS_WRAP,
            AddressV: D3D11_TEXTURE_ADDRESS_WRAP,
            AddressW: D3D11_TEXTURE_ADDRESS_WRAP,
            ComparisonFunc: D3D11_COMPARISON_NEVER,
            MinLOD: 0.0,
            MaxLOD: D3D11_FLOAT32_MAX,
            ..Default::default()
        };

        let sampler_state = device.CreateSamplerState(&sampler_desc)?;

        let indices = [0, 1, 2, 0, 2, 3];

        let buffer_desc = D3D11_BUFFER_DESC {
            ByteWidth: std::mem::size_of_val(&indices) as u32,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_INDEX_BUFFER,
            ..Default::default()
        };

        let subresource_data = D3D11_SUBRESOURCE_DATA {
            pSysMem: indices.as_ptr() as *const _,
            ..Default::default()
        };

        let index_buffer = device.CreateBuffer(&buffer_desc, Some(&subresource_data))?;


        let viewport = D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: (rect.right - rect.left) as f32,
            Height: (rect.bottom - rect.top) as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };

        context.RSSetViewports(Some(&[viewport]));

        Ok(D3D11State {
            device,
            context,
            swap_chain,
            render_target_view,
            texture,
            vertex_shader,
            pixel_shader,
            input_layout,
            vertex_buffer,
            index_buffer,
            sampler_state,
        })
    }
}

extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message {
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}
