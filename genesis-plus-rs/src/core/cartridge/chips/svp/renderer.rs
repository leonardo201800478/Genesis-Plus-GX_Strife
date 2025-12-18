//! Renderizador 3D do SVP (Sega Virtua Processor)
//! Renderiza polígonos 3D para o Virtua Racing
//! Baseado em observações do código original e documentação do SVP

use super::texture::TextureUnit;
use log::{trace, warn, debug, info};
use std::f32::consts::PI;

/// Modo de renderização
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    Wireframe,      // Apenas wireframe
    Solid,          // Sólido sem textura
    Textured,       // Texturizado
    TexturedLight,  // Texturizado com iluminação
}

/// Buffer Z (profundidade)
pub struct ZBuffer {
    width: u16,
    height: u16,
    data: Vec<f32>,  // Valores de profundidade (0.0 a 1.0, onde 0.0 é mais próximo)
    enabled: bool,
    write_enabled: bool,
    test_enabled: bool,
    near_plane: f32,
    far_plane: f32,
}

/// Framebuffer de saída
pub struct FrameBuffer {
    width: u16,
    height: u16,
    data: Vec<u16>,  // Pixels em formato RGB555
    clear_color: u16,
    double_buffered: bool,
    front_buffer: usize,
    back_buffer: usize,
    buffers: [Vec<u16>; 2],  // Para double buffering
}

/// Vértice 3D
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,       // Coordenada homogênea
    pub u: f32,       // Coordenada de textura U
    pub v: f32,       // Coordenada de textura V
    pub color: u16,   // Cor do vértice (RGB555)
    pub intensity: f32, // Intensidade para iluminação
}

/// Polígono (triângulo ou quadrilátero)
#[derive(Debug, Clone)]
pub struct Polygon {
    pub vertices: Vec<Vertex>,
    pub texture_id: u8,
    pub color: u16,
    pub flags: u16,
    pub z_min: f32,
    pub z_max: f32,
}

/// Projeção de câmera
pub struct Camera {
    pub position: [f32; 3],
    pub rotation: [f32; 3],  // Euler angles (pitch, yaw, roll)
    pub fov: f32,            // Field of view em graus
    pub aspect_ratio: f32,
    pub view_matrix: [[f32; 4]; 4],
    pub projection_matrix: [[f32; 4]; 4],
}

/// Renderizador principal do SVP
pub struct SVPRenderer {
    framebuffer: FrameBuffer,
    zbuffer: ZBuffer,
    camera: Camera,
    viewport: (u16, u16, u16, u16), // x, y, width, height
    render_mode: RenderMode,
    fog_enabled: bool,
    fog_color: u16,
    fog_start: f32,
    fog_end: f32,
    light_direction: [f32; 3],
    ambient_light: f32,
    diffuse_light: f32,
    polygon_count: u32,
    vertex_count: u32,
    triangle_count: u32,
    quad_count: u32,
}

impl ZBuffer {
    /// Cria um novo Z-buffer
    pub fn new(width: u16, height: u16) -> Self {
        let size = width as usize * height as usize;
        
        Self {
            width,
            height,
            data: vec![1.0; size], // Inicializa com valor mais distante
            enabled: true,
            write_enabled: true,
            test_enabled: true,
            near_plane: 0.1,
            far_plane: 1000.0,
        }
    }
    
    /// Limpa o Z-buffer
    pub fn clear(&mut self, value: f32) {
        self.data.fill(value);
    }
    
    /// Testa e escreve um valor de profundidade
    pub fn test_and_write(&mut self, x: u16, y: u16, depth: f32) -> bool {
        if !self.enabled || !self.test_enabled {
            return true;
        }
        
        let idx = y as usize * self.width as usize + x as usize;
        if idx < self.data.len() {
            // No SVP, quanto menor o valor, mais próximo (0.0 = mais próximo)
            if depth < self.data[idx] {
                if self.write_enabled {
                    self.data[idx] = depth;
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    }
    
    /// Obtém o valor de profundidade em uma posição
    pub fn get_depth(&self, x: u16, y: u16) -> f32 {
        let idx = y as usize * self.width as usize + x as usize;
        if idx < self.data.len() {
            self.data[idx]
        } else {
            1.0
        }
    }
    
    /// Habilita/desabilita o Z-buffer
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    
    /// Habilita/desabilita escrita no Z-buffer
    pub fn set_write_enabled(&mut self, enabled: bool) {
        self.write_enabled = enabled;
    }
    
    /// Habilita/desabilita teste de profundidade
    pub fn set_test_enabled(&mut self, enabled: bool) {
        self.test_enabled = enabled;
    }
    
    /// Define os planos near/far
    pub fn set_planes(&mut self, near: f32, far: f32) {
        self.near_plane = near;
        self.far_plane = far;
    }
    
    /// Converte valor Z do espaço de visualização para valor de profundidade [0, 1]
    pub fn linearize_depth(&self, z_view: f32) -> f32 {
        // Conversão linear (pode ser perspectiva-correct)
        (z_view - self.near_plane) / (self.far_plane - self.near_plane)
    }
}

impl FrameBuffer {
    /// Cria um novo framebuffer
    pub fn new(width: u16, height: u16, double_buffered: bool) -> Self {
        let size = width as usize * height as usize;
        let clear_color = 0x0000; // Preto em RGB555
        
        let mut buffers = [
            vec![clear_color; size],
            vec![clear_color; size],
        ];
        
        Self {
            width,
            height,
            data: buffers[0].clone(),
            clear_color,
            double_buffered,
            front_buffer: 0,
            back_buffer: 1,
            buffers,
        }
    }
    
    /// Limpa o framebuffer
    pub fn clear(&mut self) {
        if self.double_buffered {
            self.buffers[self.back_buffer].fill(self.clear_color);
        } else {
            self.data.fill(self.clear_color);
        }
    }
    
    /// Desenha um pixel
    pub fn draw_pixel(&mut self, x: u16, y: u16, color: u16) -> bool {
        if x < self.width && y < self.height {
            let idx = y as usize * self.width as usize + x as usize;
            
            if self.double_buffered {
                if idx < self.buffers[self.back_buffer].len() {
                    self.buffers[self.back_buffer][idx] = color;
                    return true;
                }
            } else {
                if idx < self.data.len() {
                    self.data[idx] = color;
                    return true;
                }
            }
        }
        false
    }
    
    /// Obtém um pixel
    pub fn get_pixel(&self, x: u16, y: u16) -> u16 {
        if x < self.width && y < self.height {
            let idx = y as usize * self.width as usize + x as usize;
            
            if self.double_buffered {
                if idx < self.buffers[self.front_buffer].len() {
                    return self.buffers[self.front_buffer][idx];
                }
            } else {
                if idx < self.data.len() {
                    return self.data[idx];
                }
            }
        }
        0x0000
    }
    
    /// Troca buffers (se double buffered)
    pub fn swap_buffers(&mut self) {
        if self.double_buffered {
            std::mem::swap(&mut self.front_buffer, &mut self.back_buffer);
            self.data = self.buffers[self.front_buffer].clone();
        }
    }
    
    /// Retorna o framebuffer atual
    pub fn get_buffer(&self) -> &[u16] {
        if self.double_buffered {
            &self.buffers[self.front_buffer]
        } else {
            &self.data
        }
    }
    
    /// Retorna o buffer de back (para escrita)
    pub fn get_back_buffer(&mut self) -> &mut [u16] {
        if self.double_buffered {
            &mut self.buffers[self.back_buffer]
        } else {
            &mut self.data
        }
    }
    
    /// Define a cor de limpeza
    pub fn set_clear_color(&mut self, color: u16) {
        self.clear_color = color;
    }
    
    /// Obtém a resolução
    pub fn get_resolution(&self) -> (u16, u16) {
        (self.width, self.height)
    }
}

impl Camera {
    /// Cria uma nova câmera
    pub fn new(fov: f32, aspect_ratio: f32) -> Self {
        let mut camera = Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            fov,
            aspect_ratio,
            view_matrix: [[0.0; 4]; 4],
            projection_matrix: [[0.0; 4]; 4],
        };
        
        camera.update_view_matrix();
        camera.update_projection_matrix();
        
        camera
    }
    
    /// Atualiza a matriz de visualização
    pub fn update_view_matrix(&mut self) {
        // Converte ângulos de Euler para radianos
        let pitch = self.rotation[0] * PI / 180.0;
        let yaw = self.rotation[1] * PI / 180.0;
        let roll = self.rotation[2] * PI / 180.0;
        
        // Cálculo da matriz de rotação (simplificado)
        let (sin_pitch, cos_pitch) = pitch.sin_cos();
        let (sin_yaw, cos_yaw) = yaw.sin_cos();
        let (sin_roll, cos_roll) = roll.sin_cos();
        
        // Matriz de rotação combinada
        self.view_matrix = [
            [
                cos_yaw * cos_roll,
                -cos_yaw * sin_roll,
                sin_yaw,
                0.0,
            ],
            [
                sin_pitch * sin_yaw * cos_roll + cos_pitch * sin_roll,
                -sin_pitch * sin_yaw * sin_roll + cos_pitch * cos_roll,
                -sin_pitch * cos_yaw,
                0.0,
            ],
            [
                -cos_pitch * sin_yaw * cos_roll + sin_pitch * sin_roll,
                cos_pitch * sin_yaw * sin_roll + sin_pitch * cos_roll,
                cos_pitch * cos_yaw,
                0.0,
            ],
            [
                -self.position[0],
                -self.position[1],
                -self.position[2],
                1.0,
            ],
        ];
    }
    
    /// Atualiza a matriz de projeção
    pub fn update_projection_matrix(&mut self) {
        let fov_rad = self.fov * PI / 180.0;
        let f = 1.0 / (fov_rad / 2.0).tan();
        let range_inv = 1.0 / (0.1 - 1000.0); // near/far inverso
        
        self.projection_matrix = [
            [f / self.aspect_ratio, 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [0.0, 0.0, (0.1 + 1000.0) * range_inv, -1.0],
            [0.0, 0.0, 0.1 * 1000.0 * range_inv * 2.0, 0.0],
        ];
    }
    
    /// Define a posição da câmera
    pub fn set_position(&mut self, x: f32, y: f32, z: f32) {
        self.position = [x, y, z];
        self.update_view_matrix();
    }
    
    /// Define a rotação da câmera
    pub fn set_rotation(&mut self, pitch: f32, yaw: f32, roll: f32) {
        self.rotation = [pitch, yaw, roll];
        self.update_view_matrix();
    }
    
    /// Transforma um vértice do espaço mundial para espaço de tela
    pub fn transform_vertex(&self, vertex: &Vertex) -> Vertex {
        let mut result = *vertex;
        
        // Aplica matriz de visualização
        let x = vertex.x;
        let y = vertex.y;
        let z = vertex.z;
        let w = vertex.w;
        
        // Multiplicação por matriz de visualização (simplificada)
        let view_x = self.view_matrix[0][0] * x + self.view_matrix[1][0] * y + 
                    self.view_matrix[2][0] * z + self.view_matrix[3][0] * w;
        let view_y = self.view_matrix[0][1] * x + self.view_matrix[1][1] * y + 
                    self.view_matrix[2][1] * z + self.view_matrix[3][1] * w;
        let view_z = self.view_matrix[0][2] * x + self.view_matrix[1][2] * y + 
                    self.view_matrix[2][2] * z + self.view_matrix[3][2] * w;
        let view_w = self.view_matrix[0][3] * x + self.view_matrix[1][3] * y + 
                    self.view_matrix[2][3] * z + self.view_matrix[3][3] * w;
        
        // Multiplicação por matriz de projeção
        let proj_x = self.projection_matrix[0][0] * view_x + 
                    self.projection_matrix[1][0] * view_y + 
                    self.projection_matrix[2][0] * view_z + 
                    self.projection_matrix[3][0] * view_w;
        let proj_y = self.projection_matrix[0][1] * view_x + 
                    self.projection_matrix[1][1] * view_y + 
                    self.projection_matrix[2][1] * view_z + 
                    self.projection_matrix[3][1] * view_w;
        let proj_z = self.projection_matrix[0][2] * view_x + 
                    self.projection_matrix[1][2] * view_y + 
                    self.projection_matrix[2][2] * view_z + 
                    self.projection_matrix[3][2] * view_w;
        let proj_w = self.projection_matrix[0][3] * view_x + 
                    self.projection_matrix[1][3] * view_y + 
                    self.projection_matrix[2][3] * view_z + 
                    self.projection_matrix[3][3] * view_w;
        
        // Divisão perspectiva
        if proj_w != 0.0 {
            result.x = proj_x / proj_w;
            result.y = proj_y / proj_w;
            result.z = proj_z / proj_w;
            result.w = proj_w;
            
            // Preserva coordenadas de textura com correção perspectiva
            result.u = vertex.u / proj_w;
            result.v = vertex.v / proj_w;
        }
        
        result
    }
}

impl SVPRenderer {
    /// Cria um novo renderizador
    pub fn new() -> Self {
        let width = 320;  // Resolução típica do Mega Drive
        let height = 224;
        
        info!("Inicializando SVPRenderer: {}x{}", width, height);
        
        Self {
            framebuffer: FrameBuffer::new(width, height, true),
            zbuffer: ZBuffer::new(width, height),
            camera: Camera::new(60.0, width as f32 / height as f32),
            viewport: (0, 0, width, height),
            render_mode: RenderMode::Textured,
            fog_enabled: false,
            fog_color: 0x7C00, // Vermelho para debug
            fog_start: 100.0,
            fog_end: 500.0,
            light_direction: [0.0, -1.0, 0.0],
            ambient_light: 0.3,
            diffuse_light: 0.7,
            polygon_count: 0,
            vertex_count: 0,
            triangle_count: 0,
            quad_count: 0,
        }
    }
    
    /// Reseta o renderizador
    pub fn reset(&mut self) {
        self.framebuffer.clear();
        self.zbuffer.clear(1.0);
        self.polygon_count = 0;
        self.vertex_count = 0;
        self.triangle_count = 0;
        self.quad_count = 0;
        
        info!("SVPRenderer resetado");
    }
    
    /// Limpa o framebuffer e z-buffer
    pub fn clear(&mut self) {
        self.framebuffer.clear();
        self.zbuffer.clear(1.0);
    }
    
    /// Desenha uma linha
    pub fn draw_line(&mut self, v1: Vertex, v2: Vertex, color: u16) {
        // Transforma vértices para espaço de tela
        let v1_screen = self.world_to_screen(&v1);
        let v2_screen = self.world_to_screen(&v2);
        
        // Algoritmo de Bresenham para linhas
        let mut x0 = v1_screen.x as i32;
        let mut y0 = v1_screen.y as i32;
        let x1 = v2_screen.x as i32;
        let y1 = v2_screen.y as i32;
        
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        
        loop {
            // Desenha pixel atual
            if x0 >= 0 && x0 < self.framebuffer.width as i32 &&
               y0 >= 0 && y0 < self.framebuffer.height as i32 {
                let depth = self.calculate_depth(&v1_screen, &v2_screen, x0, y0);
                
                if self.zbuffer.test_and_write(x0 as u16, y0 as u16, depth) {
                    self.framebuffer.draw_pixel(x0 as u16, y0 as u16, color);
                }
            }
            
            if x0 == x1 && y0 == y1 {
                break;
            }
            
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
        
        self.polygon_count += 1;
    }
    
    /// Desenha um polígono
    pub fn draw_polygon(
        &mut self, 
        vertices: &[Vertex],
        texture_unit: &TextureUnit,
        texture_id: u8,
        color: u16,
    ) {
        if vertices.len() < 3 {
            warn!("Polígono com menos de 3 vértices ignorado");
            return;
        }
        
        // Transforma todos os vértices para espaço de tela
        let screen_vertices: Vec<Vertex> = vertices.iter()
            .map(|v| self.world_to_screen(v))
            .collect();
        
        // Verifica se o polígono é visível (backface culling simplificado)
        if !self.is_front_facing(&screen_vertices) {
            trace!("Polígono backface culled");
            return;
        }
        
        // Determina o bounding box do polígono
        let (min_x, max_x, min_y, max_y) = self.calculate_bounding_box(&screen_vertices);
        
        // Para cada pixel no bounding box
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                if x >= 0 && x < self.framebuffer.width as i32 &&
                   y >= 0 && y < self.framebuffer.height as i32 {
                    
                    // Testa se o ponto está dentro do polígono
                    if self.is_point_in_polygon(x, y, &screen_vertices) {
                        // Calcula coordenadas baricêntricas
                        let (alpha, beta, gamma) = self.calculate_barycentric(
                            x, y, &screen_vertices
                        );
                        
                        // Interpola profundidade
                        let depth = self.interpolate_depth(alpha, beta, gamma, &screen_vertices);
                        
                        // Testa profundidade
                        if self.zbuffer.test_and_write(x as u16, y as u16, depth) {
                            // Calcula cor do pixel
                            let pixel_color = match self.render_mode {
                                RenderMode::Wireframe => color,
                                RenderMode::Solid => color,
                                RenderMode::Textured | RenderMode::TexturedLight => {
                                    self.calculate_textured_pixel(
                                        alpha, beta, gamma, 
                                        &screen_vertices, 
                                        texture_unit, 
                                        texture_id
                                    )
                                }
                            };
                            
                            // Aplica névoa se habilitada
                            let final_color = if self.fog_enabled {
                                self.apply_fog(pixel_color, depth)
                            } else {
                                pixel_color
                            };
                            
                            // Desenha o pixel
                            self.framebuffer.draw_pixel(x as u16, y as u16, final_color);
                        }
                    }
                }
            }
        }
        
        self.polygon_count += 1;
        self.vertex_count += vertices.len() as u32;
        
        match vertices.len() {
            3 => self.triangle_count += 1,
            4 => self.quad_count += 1,
            _ => {}
        }
    }
    
    /// Desenha um triângulo (wrapper para draw_polygon)
    pub fn draw_triangle(
        &mut self,
        v1: Vertex,
        v2: Vertex,
        v3: Vertex,
        texture_unit: &TextureUnit,
        texture_id: u8,
        color: u16,
    ) {
        self.draw_polygon(&[v1, v2, v3], texture_unit, texture_id, color);
    }
    
    /// Desenha um quadrilátero (como dois triângulos)
    pub fn draw_quad(
        &mut self,
        v1: Vertex,
        v2: Vertex,
        v3: Vertex,
        v4: Vertex,
        texture_unit: &TextureUnit,
        texture_id: u8,
        color: u16,
    ) {
        // Divide em dois triângulos
        self.draw_triangle(v1, v2, v3, texture_unit, texture_id, color);
        self.draw_triangle(v1, v3, v4, texture_unit, texture_id, color);
    }
    
    /// Converte vértice do espaço mundial para espaço de tela
    fn world_to_screen(&self, vertex: &Vertex) -> Vertex {
        // Primeiro transforma pela câmera
        let mut transformed = self.camera.transform_vertex(vertex);
        
        // Converte para coordenadas de tela
        let (vp_x, vp_y, vp_w, vp_h) = self.viewport;
        
        // Converte de espaço NDC (-1 a 1) para pixels
        transformed.x = (transformed.x * 0.5 + 0.5) * vp_w as f32 + vp_x as f32;
        transformed.y = (1.0 - (transformed.y * 0.5 + 0.5)) * vp_h as f32 + vp_y as f32;
        
        transformed
    }
    
    /// Calcula bounding box do polígono
    fn calculate_bounding_box(&self, vertices: &[Vertex]) -> (i32, i32, i32, i32) {
        let mut min_x = i32::MAX;
        let mut max_x = i32::MIN;
        let mut min_y = i32::MAX;
        let mut max_y = i32::MIN;
        
        for v in vertices {
            let x = v.x as i32;
            let y = v.y as i32;
            
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        
        // Clamp para a viewport
        let (vp_x, vp_y, vp_w, vp_h) = self.viewport;
        min_x = min_x.max(vp_x as i32);
        max_x = max_x.min((vp_x + vp_w) as i32 - 1);
        min_y = min_y.max(vp_y as i32);
        max_y = max_y.min((vp_y + vp_h) as i32 - 1);
        
        (min_x, max_x, min_y, max_y)
    }
    
    /// Verifica se um polígono está voltado para a frente (backface culling)
    fn is_front_facing(&self, vertices: &[Vertex]) -> bool {
        if vertices.len() < 3 {
            return false;
        }
        
        // Calcula a normal do polígono usando os primeiros 3 vértices
        let v0 = &vertices[0];
        let v1 = &vertices[1];
        let v2 = &vertices[2];
        
        let edge1_x = v1.x - v0.x;
        let edge1_y = v1.y - v0.y;
        let edge2_x = v2.x - v0.x;
        let edge2_y = v2.y - v0.y;
        
        // Produto cruzado em 2D (z-component)
        let cross_z = edge1_x * edge2_y - edge1_y * edge2_x;
        
        // Se cross_z > 0, o polígono está voltado para a câmera (sentido anti-horário)
        cross_z > 0.0
    }
    
    /// Testa se um ponto está dentro de um polígono
    fn is_point_in_polygon(&self, x: i32, y: i32, vertices: &[Vertex]) -> bool {
        let mut inside = false;
        let mut j = vertices.len() - 1;
        
        for i in 0..vertices.len() {
            let vi = &vertices[i];
            let vj = &vertices[j];
            
            let xi = vi.x as i32;
            let yi = vi.y as i32;
            let xj = vj.x as i32;
            let yj = vj.y as i32;
            
            // Teste de interseção de raio
            if ((yi > y) != (yj > y)) && 
               (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
                inside = !inside;
            }
            
            j = i;
        }
        
        inside
    }
    
    /// Calcula coordenadas baricêntricas
    fn calculate_barycentric(&self, x: i32, y: i32, vertices: &[Vertex]) -> (f32, f32, f32) {
        // Para triângulos (assumimos polígonos triangulados)
        let v0 = &vertices[0];
        let v1 = &vertices[1];
        let v2 = &vertices[2];
        
        let x0 = v0.x;
        let y0 = v0.y;
        let x1 = v1.x;
        let y1 = v1.y;
        let x2 = v2.x;
        let y2 = v2.y;
        
        let x_f = x as f32;
        let y_f = y as f32;
        
        // Fórmula baricêntrica
        let denom = (y1 - y2) * (x0 - x2) + (x2 - x1) * (y0 - y2);
        
        if denom.abs() < 0.0001 {
            return (0.0, 0.0, 0.0);
        }
        
        let alpha = ((y1 - y2) * (x_f - x2) + (x2 - x1) * (y_f - y2)) / denom;
        let beta = ((y2 - y0) * (x_f - x2) + (x0 - x2) * (y_f - y2)) / denom;
        let gamma = 1.0 - alpha - beta;
        
        (alpha, beta, gamma)
    }
    
    /// Interpola profundidade usando coordenadas baricêntricas
    fn interpolate_depth(&self, alpha: f32, beta: f32, gamma: f32, vertices: &[Vertex]) -> f32 {
        let v0 = &vertices[0];
        let v1 = &vertices[1];
        let v2 = &vertices[2];
        
        // Interpolação linear de Z (perspective-correct seria 1/z interpolação)
        alpha * v0.z + beta * v1.z + gamma * v2.z
    }
    
    /// Calcula profundidade ao longo de uma linha
    fn calculate_depth(&self, v1: &Vertex, v2: &Vertex, x: i32, y: i32) -> f32 {
        // Interpolação linear ao longo da linha
        let dx = v2.x - v1.x;
        let dy = v2.y - v1.y;
        let length_sq = dx * dx + dy * dy;
        
        if length_sq < 0.0001 {
            return v1.z;
        }
        
        let t = ((x as f32 - v1.x) * dx + (y as f32 - v1.y) * dy) / length_sq;
        let t_clamped = t.clamp(0.0, 1.0);
        
        v1.z * (1.0 - t_clamped) + v2.z * t_clamped
    }
    
    /// Calcula cor de pixel texturizado
    fn calculate_textured_pixel(
        &self,
        alpha: f32,
        beta: f32,
        gamma: f32,
        vertices: &[Vertex],
        texture_unit: &TextureUnit,
        texture_id: u8,
    ) -> u16 {
        // Interpola coordenadas de textura com correção perspectiva
        let v0 = &vertices[0];
        let v1 = &vertices[1];
        let v2 = &vertices[2];
        
        // Interpola 1/w para correção perspectiva
        let w0 = if v0.w != 0.0 { 1.0 / v0.w } else { 1.0 };
        let w1 = if v1.w != 0.0 { 1.0 / v1.w } else { 1.0 };
        let w2 = if v2.w != 0.0 { 1.0 / v2.w } else { 1.0 };
        
        let inv_w = alpha * w0 + beta * w1 + gamma * w2;
        
        if inv_w == 0.0 {
            return 0x7C00; // Vermelho para debug
        }
        
        // Interpola u/w e v/w
        let u_over_w = alpha * v0.u * w0 + beta * v1.u * w1 + gamma * v2.u * w2;
        let v_over_w = alpha * v0.v * w0 + beta * v1.v * w1 + gamma * v2.v * w2;
        
        // Divide por inv_w para obter u, v corretos
        let u = u_over_w / inv_w;
        let v = v_over_w / inv_w;
        
        // Amostra a textura
        let tex_color = texture_unit.sample(u, v, 0.0);
        
        // Aplica iluminação se habilitado
        if self.render_mode == RenderMode::TexturedLight {
            self.apply_lighting(tex_color, alpha, beta, gamma, vertices)
        } else {
            tex_color
        }
    }
    
    /// Aplica iluminação à cor
    fn apply_lighting(&self, color: u16, alpha: f32, beta: f32, gamma: f32, vertices: &[Vertex]) -> u16 {
        // Interpola intensidade dos vértices
        let v0 = &vertices[0];
        let v1 = &vertices[1];
        let v2 = &vertices[2];
        
        let intensity = alpha * v0.intensity + beta * v1.intensity + gamma * v2.intensity;
        
        // Aplica intensidade à cor (simplificado)
        self.multiply_color(color, intensity.clamp(0.0, 1.0))
    }
    
    /// Multiplica cor por um fator
    fn multiply_color(&self, color: u16, factor: f32) -> u16 {
        let r = ((color >> 10) & 0x1F) as f32;
        let g = ((color >> 5) & 0x1F) as f32;
        let b = (color & 0x1F) as f32;
        
        let r_new = (r * factor).clamp(0.0, 31.0) as u16;
        let g_new = (g * factor).clamp(0.0, 31.0) as u16;
        let b_new = (b * factor).clamp(0.0, 31.0) as u16;
        
        (r_new << 10) | (g_new << 5) | b_new
    }
    
    /// Aplica efeito de névoa
    fn apply_fog(&self, color: u16, depth: f32) -> u16 {
        // Calcula fator de névoa baseado na profundidade
        let fog_factor = ((depth - self.fog_start) / (self.fog_end - self.fog_start))
            .clamp(0.0, 1.0);
        
        // Interpola linearmente entre a cor original e a cor da névoa
        self.interpolate_colors(color, self.fog_color, fog_factor)
    }
    
    /// Interpola entre duas cores
    fn interpolate_colors(&self, color1: u16, color2: u16, t: f32) -> u16 {
        let r1 = ((color1 >> 10) & 0x1F) as f32;
        let g1 = ((color1 >> 5) & 0x1F) as f32;
        let b1 = (color1 & 0x1F) as f32;
        
        let r2 = ((color2 >> 10) & 0x1F) as f32;
        let g2 = ((color2 >> 5) & 0x1F) as f32;
        let b2 = (color2 & 0x1F) as f32;
        
        let r = r1 * (1.0 - t) + r2 * t;
        let g = g1 * (1.0 - t) + g2 * t;
        let b = b1 * (1.0 - t) + b2 * t;
        
        ((r as u16) << 10) | ((g as u16) << 5) | (b as u16)
    }
    
    /// Troca buffers (double buffering)
    pub fn swap_buffers(&mut self) {
        self.framebuffer.swap_buffers();
    }
    
    /// Retorna o framebuffer atual
    pub fn get_frame_buffer(&self) -> &[u16] {
        self.framebuffer.get_buffer()
    }
    
    /// Define o modo de renderização
    pub fn set_render_mode(&mut self, mode: RenderMode) {
        self.render_mode = mode;
        debug!("Modo de renderização definido para: {:?}", mode);
    }
    
    /// Habilita/desabilita névoa
    pub fn set_fog_enabled(&mut self, enabled: bool) {
        self.fog_enabled = enabled;
    }
    
    /// Define parâmetros da névoa
    pub fn set_fog_params(&mut self, color: u16, start: f32, end: f32) {
        self.fog_color = color;
        self.fog_start = start;
        self.fog_end = end;
    }
    
    /// Define a viewport
    pub fn set_viewport(&mut self, x: u16, y: u16, width: u16, height: u16) {
        self.viewport = (x, y, width, height);
    }
    
    /// Define a direção da luz
    pub fn set_light_direction(&mut self, x: f32, y: f32, z: f32) {
        self.light_direction = [x, y, z];
    }
    
    /// Define intensidades de luz
    pub fn set_light_intensities(&mut self, ambient: f32, diffuse: f32) {
        self.ambient_light = ambient;
        self.diffuse_light = diffuse;
    }
    
    /// Retorna estatísticas de renderização
    pub fn get_stats(&self) -> (u32, u32, u32, u32) {
        (
            self.polygon_count,
            self.vertex_count,
            self.triangle_count,
            self.quad_count,
        )
    }
    
    /// Reseta estatísticas
    pub fn reset_stats(&mut self) {
        self.polygon_count = 0;
        self.vertex_count = 0;
        self.triangle_count = 0;
        self.quad_count = 0;
    }
}