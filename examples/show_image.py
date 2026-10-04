import base64
import sys
import zlib
import struct

def create_sample_png():
    """Genera una imagen PNG de 120x80 px con un gradiente y una curva de pérdida (loss) simulada."""
    width, height = 120, 80
    raw_data = bytearray()
    
    for y in range(height):
        raw_data.append(0)  # Filtro PNG: None
        # Simulación de curva de entrenamiento
        expected_y = int(15 + 45 / (1 + (y * 0.08)))
        for x in range(width):
            curve_y = int(60 - 45 * (1 - (x / width)**0.5))
            if abs(y - curve_y) <= 1:
                # Línea verde de la curva
                raw_data.extend((80, 230, 120, 255))
            else:
                # Fondo oscuro gradiente estilo Ghostty
                r = int(24 + (x / width) * 20)
                g = int(26 + (y / height) * 20)
                b = int(35 + (x / width) * 40)
                raw_data.extend((r, g, b, 255))
                
    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xffffffff)

    header = b"\x89PNG\r\n\x1a\n"
    ihdr = chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
    idat = chunk(b"IDAT", zlib.compress(bytes(raw_data)))
    iend = chunk(b"IEND", b"")
    return header + ihdr + idat + iend

def show_image_in_axon(png_bytes):
    b64 = base64.b64encode(png_bytes).decode("ascii")
    # Protocolo de imagen OSC 1337 nativo de Axon
    sys.stdout.write(f"\033]1337;File=inline=1:{b64}\a\n")
    sys.stdout.flush()

if __name__ == "__main__":
    png = create_sample_png()
    print("--- [Axon Image Protocol Test: Curva de Entrenamiento PyTorch / Loss] ---")
    show_image_in_axon(png)
    print("Imagen renderizada nativamente por GPU dentro del buffer de Axon.")
