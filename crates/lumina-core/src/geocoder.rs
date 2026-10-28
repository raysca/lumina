use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeoLocation {
    pub city: String,
    pub admin1_region: String,
    pub country_code: String,
}

#[derive(Debug, Clone)]
struct CityPoint {
    point: [f32; 3], // Unit sphere 3D vector [x, y, z]
    city: &'static str,
    admin1: &'static str,
    country: &'static str,
}

fn to_sphere_point(lat: f64, lon: f64) -> [f32; 3] {
    let lat_rad = lat.to_radians();
    let lon_rad = lon.to_radians();
    let x = (lat_rad.cos() * lon_rad.cos()) as f32;
    let y = (lat_rad.cos() * lon_rad.sin()) as f32;
    let z = lat_rad.sin() as f32;
    [x, y, z]
}

#[inline(always)]
fn sq_dist(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}

#[derive(Clone)]
pub struct OfflineGeocoder {
    points: Arc<Vec<CityPoint>>,
}

impl OfflineGeocoder {
    pub fn new() -> Self {
        let cities = load_default_cities();
        let points: Vec<CityPoint> = cities
            .into_iter()
            .map(|(name, admin, country, lat, lon)| CityPoint {
                point: to_sphere_point(lat, lon),
                city: name,
                admin1: admin,
                country,
            })
            .collect();

        Self {
            points: Arc::new(points),
        }
    }

    /// Resolves nearest city in < 1 microsecond (within ~250 km maximum threshold)
    pub fn reverse_geocode(&self, lat: f64, lon: f64) -> Option<GeoLocation> {
        if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
            return None;
        }
        let target = to_sphere_point(lat, lon);
        // Max chord distance squared on unit sphere for ~250 km: (250 / 6371)^2 ≈ 0.00154
        const MAX_SQ_DIST: f32 = 0.0016;
        self.points
            .iter()
            .map(|pt| (pt, sq_dist(&target, &pt.point)))
            .filter(|(_, d)| *d <= MAX_SQ_DIST)
            .min_by(|(_, da), (_, db)| da.partial_cmp(db).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(pt, _)| GeoLocation {
                city: pt.city.to_string(),
                admin1_region: pt.admin1.to_string(),
                country_code: pt.country.to_string(),
            })
    }
}

impl Default for OfflineGeocoder {
    fn default() -> Self {
        Self::new()
    }
}

fn load_default_cities() -> Vec<(&'static str, &'static str, &'static str, f64, f64)> {
    vec![
        // North America - United States
        ("New York", "New York", "US", 40.7128, -74.0060),
        ("Los Angeles", "California", "US", 34.0522, -118.2437),
        ("San Francisco", "California", "US", 37.7749, -122.4194),
        ("San Jose", "California", "US", 37.3382, -121.8863),
        ("San Diego", "California", "US", 32.7157, -117.1611),
        ("Seattle", "Washington", "US", 47.6062, -122.3321),
        ("Portland", "Oregon", "US", 45.5152, -122.6784),
        ("Chicago", "Illinois", "US", 41.8781, -87.6298),
        ("Houston", "Texas", "US", 29.7604, -95.3698),
        ("Austin", "Texas", "US", 30.2672, -97.7431),
        ("Dallas", "Texas", "US", 32.7767, -96.7970),
        ("Miami", "Florida", "US", 25.7617, -80.1918),
        ("Orlando", "Florida", "US", 28.5383, -81.3792),
        ("Atlanta", "Georgia", "US", 33.7490, -84.3880),
        ("Boston", "Massachusetts", "US", 42.3601, -71.0589),
        (
            "Washington",
            "District of Columbia",
            "US",
            38.9072,
            -77.0369,
        ),
        ("Denver", "Colorado", "US", 39.7392, -104.9903),
        ("Phoenix", "Arizona", "US", 33.4484, -112.0740),
        ("Las Vegas", "Nevada", "US", 36.1699, -115.1398),
        ("Honolulu", "Hawaii", "US", 21.3069, -157.8583),
        ("Anchorage", "Alaska", "US", 61.2181, -149.9003),
        ("Philadelphia", "Pennsylvania", "US", 39.9526, -75.1652),
        ("Minneapolis", "Minnesota", "US", 44.9778, -93.2650),
        ("Nashville", "Tennessee", "US", 36.1627, -86.7816),
        ("New Orleans", "Louisiana", "US", 29.9511, -90.0715),
        ("Detroit", "Michigan", "US", 42.3314, -83.0458),
        ("Salt Lake City", "Utah", "US", 40.7608, -111.8910),
        // North America - Canada
        ("Toronto", "Ontario", "CA", 43.6532, -79.3832),
        ("Montreal", "Quebec", "CA", 45.5017, -73.5673),
        ("Vancouver", "British Columbia", "CA", 49.2827, -123.1207),
        ("Calgary", "Alberta", "CA", 51.0447, -114.0719),
        ("Ottawa", "Ontario", "CA", 45.4215, -75.6972),
        ("Quebec City", "Quebec", "CA", 46.8139, -71.2080),
        ("Victoria", "British Columbia", "CA", 48.4284, -123.3656),
        ("Halifax", "Nova Scotia", "CA", 44.6488, -63.5752),
        ("Banff", "Alberta", "CA", 51.1784, -115.5708),
        // Europe - United Kingdom & Ireland
        ("London", "England", "GB", 51.5074, -0.1278),
        ("Manchester", "England", "GB", 53.4808, -2.2426),
        ("Birmingham", "England", "GB", 52.4862, -1.8904),
        ("Edinburgh", "Scotland", "GB", 55.9533, -3.1883),
        ("Glasgow", "Scotland", "GB", 55.8642, -4.2518),
        ("Liverpool", "England", "GB", 53.4084, -2.9916),
        ("Oxford", "England", "GB", 51.7520, -1.2577),
        ("Cambridge", "England", "GB", 52.2053, 0.1218),
        ("Dublin", "Leinster", "IE", 53.3498, -6.2603),
        ("Belfast", "Northern Ireland", "GB", 54.5973, -5.9301),
        // Europe - France
        ("Paris", "Île-de-France", "FR", 48.8566, 2.3522),
        ("Nice", "Provence-Alpes-Côte d'Azur", "FR", 43.7102, 7.2620),
        (
            "Marseille",
            "Provence-Alpes-Côte d'Azur",
            "FR",
            43.2965,
            5.3698,
        ),
        ("Lyon", "Auvergne-Rhône-Alpes", "FR", 45.7640, 4.8357),
        ("Bordeaux", "Nouvelle-Aquitaine", "FR", 44.8378, -0.5792),
        ("Toulouse", "Occitanie", "FR", 43.6047, 1.4442),
        ("Strasbourg", "Grand Est", "FR", 48.5734, 7.7521),
        (
            "Cannes",
            "Provence-Alpes-Côte d'Azur",
            "FR",
            43.5528,
            7.0174,
        ),
        // Europe - Germany, Austria, Switzerland
        ("Berlin", "Berlin", "DE", 52.5200, 13.4050),
        ("Munich", "Bavaria", "DE", 48.1351, 11.5820),
        ("Frankfurt", "Hesse", "DE", 50.1109, 8.6821),
        ("Hamburg", "Hamburg", "DE", 53.5511, 9.9937),
        ("Cologne", "North Rhine-Westphalia", "DE", 50.9375, 6.9603),
        ("Stuttgart", "Baden-Württemberg", "DE", 48.7758, 9.1829),
        ("Vienna", "Vienna", "AT", 48.2082, 16.3738),
        ("Salzburg", "Salzburg", "AT", 47.8095, 13.0550),
        ("Zurich", "Zurich", "CH", 47.3769, 8.5417),
        ("Geneva", "Geneva", "CH", 46.2044, 6.1432),
        ("Lucerne", "Lucerne", "CH", 47.0502, 8.3093),
        ("Zermatt", "Valais", "CH", 45.9763, 7.7491),
        // Europe - Italy, Spain, Portugal
        ("Rome", "Lazio", "IT", 41.9028, 12.4964),
        ("Milan", "Lombardy", "IT", 45.4642, 9.1900),
        ("Florence", "Tuscany", "IT", 43.7696, 11.2558),
        ("Venice", "Veneto", "IT", 45.4408, 12.3155),
        ("Naples", "Campania", "IT", 40.8518, 14.2681),
        ("Amalfi", "Campania", "IT", 40.6340, 14.6027),
        ("Madrid", "Madrid", "ES", 40.4168, -3.7038),
        ("Barcelona", "Catalonia", "ES", 41.3851, 2.1734),
        ("Seville", "Andalusia", "ES", 37.3891, -5.9845),
        ("Valencia", "Valencia", "ES", 39.4699, -0.3763),
        ("Mallorca", "Balearic Islands", "ES", 39.6953, 3.0176),
        ("Lisbon", "Lisbon", "PT", 38.7223, -9.1393),
        ("Porto", "Porto", "PT", 41.1579, -8.6291),
        // Europe - Netherlands, Belgium, Nordics, Greece, Central Europe
        ("Amsterdam", "North Holland", "NL", 52.3676, 4.9041),
        ("Brussels", "Brussels", "BE", 50.8503, 4.3517),
        ("Stockholm", "Stockholm", "SE", 59.3293, 18.0686),
        ("Copenhagen", "Capital Region", "DK", 55.6761, 12.5683),
        ("Oslo", "Oslo", "NO", 59.9139, 10.7522),
        ("Helsinki", "Uusimaa", "FI", 60.1699, 24.9384),
        ("Reykjavik", "Capital Region", "IS", 64.1466, -21.9426),
        ("Athens", "Attica", "GR", 37.9838, 23.7275),
        ("Santorini", "South Aegean", "GR", 36.3932, 25.4615),
        ("Prague", "Prague", "CZ", 50.0755, 14.4378),
        ("Budapest", "Budapest", "HU", 47.4979, 19.0402),
        ("Warsaw", "Masovia", "PL", 52.2297, 21.0122),
        ("Krakow", "Lesser Poland", "PL", 50.0647, 19.9450),
        // Asia - Japan
        ("Tokyo", "Tokyo", "JP", 35.6762, 139.6503),
        ("Kyoto", "Kyoto", "JP", 35.0116, 135.7681),
        ("Osaka", "Osaka", "JP", 34.6937, 135.5023),
        ("Yokohama", "Kanagawa", "JP", 35.4437, 139.6380),
        ("Nagoya", "Aichi", "JP", 35.1815, 136.9066),
        ("Sapporo", "Hokkaido", "JP", 43.0618, 141.3545),
        ("Fukuoka", "Fukuoka", "JP", 33.5904, 130.4017),
        ("Hiroshima", "Hiroshima", "JP", 34.3853, 132.4553),
        ("Nara", "Nara", "JP", 34.6851, 135.8048),
        ("Okinawa", "Okinawa", "JP", 26.2124, 127.6809),
        // Asia - East, South & Southeast Asia
        ("Seoul", "Seoul", "KR", 37.5665, 126.9780),
        ("Busan", "Busan", "KR", 35.1796, 129.0756),
        ("Jeju", "Jeju", "KR", 33.4996, 126.5312),
        ("Beijing", "Beijing", "CN", 39.9042, 116.4074),
        ("Shanghai", "Shanghai", "CN", 31.2304, 121.4737),
        ("Hong Kong", "Hong Kong", "HK", 22.3193, 114.1694),
        ("Taipei", "Taipei", "TW", 25.0330, 121.5654),
        ("Singapore", "Singapore", "SG", 1.3521, 103.8198),
        ("Bangkok", "Bangkok", "TH", 13.7563, 100.5018),
        ("Chiang Mai", "Chiang Mai", "TH", 18.7883, 98.9853),
        ("Phuket", "Phuket", "TH", 7.8804, 98.3923),
        ("Hanoi", "Hanoi", "VN", 21.0285, 105.8542),
        (
            "Ho Chi Minh City",
            "Ho Chi Minh City",
            "VN",
            10.8231,
            106.6297,
        ),
        ("Kuala Lumpur", "Federal Territory", "MY", 3.1390, 101.6869),
        ("Bali", "Bali", "ID", -8.4095, 115.1889),
        ("Jakarta", "Jakarta", "ID", -6.2088, 106.8456),
        ("Manila", "Metro Manila", "PH", 14.5995, 120.9842),
        ("Mumbai", "Maharashtra", "IN", 19.0760, 72.8777),
        ("Delhi", "Delhi", "IN", 28.6139, 77.2090),
        ("Bengaluru", "Karnataka", "IN", 12.9716, 77.5946),
        ("Goa", "Goa", "IN", 15.2993, 74.1240),
        // Oceania - Australia & New Zealand
        ("Sydney", "New South Wales", "AU", -33.8688, 151.2093),
        ("Melbourne", "Victoria", "AU", -37.8136, 144.9631),
        ("Brisbane", "Queensland", "AU", -27.4698, 153.0251),
        ("Perth", "Western Australia", "AU", -31.9505, 115.8605),
        ("Adelaide", "South Australia", "AU", -34.9285, 138.6007),
        ("Cairns", "Queensland", "AU", -16.9186, 145.7781),
        ("Auckland", "Auckland", "NZ", -36.8485, 174.7633),
        ("Wellington", "Wellington", "NZ", -41.2865, 174.7762),
        ("Christchurch", "Canterbury", "NZ", -43.5321, 172.6362),
        ("Queenstown", "Otago", "NZ", -45.0312, 168.6626),
        // Middle East & Africa
        ("Dubai", "Dubai", "AE", 25.2048, 55.2708),
        ("Abu Dhabi", "Abu Dhabi", "AE", 24.4539, 54.3773),
        ("Doha", "Doha", "QA", 25.2854, 51.5310),
        ("Riyadh", "Riyadh", "SA", 24.7136, 46.6753),
        ("Tel Aviv", "Tel Aviv", "IL", 32.0853, 34.7818),
        ("Jerusalem", "Jerusalem", "IL", 31.7683, 35.2137),
        ("Istanbul", "Istanbul", "TR", 41.0082, 28.9784),
        ("Cairo", "Cairo", "EG", 30.0444, 31.2357),
        ("Cape Town", "Western Cape", "ZA", -33.9249, 18.4241),
        ("Johannesburg", "Gauteng", "ZA", -26.2041, 28.0473),
        ("Nairobi", "Nairobi", "KE", -1.2921, 36.8219),
        ("Marrakech", "Marrakesh-Safi", "MA", 31.6295, -7.9811),
        // Latin America
        ("Mexico City", "CDMX", "MX", 19.4326, -99.1332),
        ("Cancun", "Quintana Roo", "MX", 21.1619, -86.8515),
        ("São Paulo", "São Paulo", "BR", -23.5505, -46.6333),
        ("Rio de Janeiro", "Rio de Janeiro", "BR", -22.9068, -43.1729),
        ("Buenos Aires", "Buenos Aires", "AR", -34.6037, -58.3816),
        ("Santiago", "Santiago", "CL", -33.4489, -70.6693),
        ("Bogotá", "Bogotá", "CO", 4.7110, -74.0721),
        ("Lima", "Lima", "PE", -12.0464, -77.0428),
        ("Cusco", "Cusco", "PE", -13.5319, -71.9675),
    ]
}
