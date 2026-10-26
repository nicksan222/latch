import { brandSvg } from '@latch/ui/brand';
export function GET() { return new Response(brandSvg(), { headers: { 'Content-Type': 'image/svg+xml' } }); }
