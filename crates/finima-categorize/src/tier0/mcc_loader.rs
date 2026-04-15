use std::collections::HashMap;

/// A parsed MCC code entry from the greggles/mcc-codes JSON format.
#[derive(Debug, serde::Deserialize)]
pub struct MccCodeEntry {
    pub mcc: String,
    pub edited_description: String,
    #[serde(default)]
    pub combined_description: String,
    #[serde(default)]
    pub usda_description: String,
    #[serde(default)]
    pub irs_description: String,
    #[serde(default)]
    pub irs_reportable: String,
    #[serde(default)]
    pub id: u32,
}

/// Parse MCC codes from the greggles/mcc-codes JSON format.
///
/// Returns a map of MCC code (u16) to edited_description.
pub fn parse_mcc_json(json_data: &str) -> Result<HashMap<u16, String>, serde_json::Error> {
    let entries: Vec<MccCodeEntry> = serde_json::from_str(json_data)?;
    let mut map = HashMap::new();
    for entry in entries {
        if let Ok(code) = entry.mcc.parse::<u16>() {
            map.insert(code, entry.edited_description);
        }
    }
    Ok(map)
}

/// Map an MCC code to a (category, subcategory) tuple.
///
/// Covers the ~60 most common MCC codes seen in consumer banking.
pub fn mcc_to_category(mcc: u16) -> Option<(&'static str, &'static str)> {
    Some(match mcc {
        // Grocery Stores
        5411 => ("food_dining", "groceries"),
        5422 => ("food_dining", "groceries"), // Freezer / locker meat provisioners
        5441 => ("food_dining", "groceries"), // Candy, nut, confectionery
        5451 => ("food_dining", "groceries"), // Dairy products
        5462 => ("food_dining", "groceries"), // Bakeries

        // Restaurants & Eating
        5812 => ("food_dining", "restaurants"),
        5813 => ("food_dining", "bars_alcohol"),
        5814 => ("food_dining", "fast_food"),

        // Coffee Shops (not a standard MCC, but mapped by some processors)
        5499 => ("food_dining", "coffee_shops"),

        // Gas Stations
        5541 => ("transportation", "gas_fuel"),
        5542 => ("transportation", "gas_fuel"), // Automated fuel dispensers

        // Transportation
        4121 => ("transportation", "rideshare_taxi"),
        4111 => ("transportation", "public_transit"),
        4112 => ("transportation", "public_transit"), // Passenger railways
        4131 => ("transportation", "public_transit"), // Bus lines
        4214 => ("transportation", "other"),          // Motor freight
        7512 => ("transportation", "car_rental"),
        7513 => ("transportation", "car_rental"), // Truck rental
        4511 => ("transportation", "air_travel"),
        4582 => ("transportation", "air_travel"), // Airports

        // Auto
        5511 => ("transportation", "auto_maintenance"),
        5521 => ("transportation", "auto_maintenance"), // Used car dealers
        5531 => ("transportation", "auto_maintenance"), // Auto parts
        5532 => ("transportation", "auto_maintenance"), // Tires
        5533 => ("transportation", "auto_maintenance"), // Auto parts
        7531 => ("transportation", "auto_maintenance"), // Auto body repair
        7534 => ("transportation", "auto_maintenance"), // Tire re-treading
        7535 => ("transportation", "auto_maintenance"), // Paint shops
        7538 => ("transportation", "auto_maintenance"), // Auto service shops
        7542 => ("transportation", "auto_maintenance"), // Car washes
        5571 => ("transportation", "auto_maintenance"), // Motorcycle dealers

        // Utilities
        4812 => ("bills_utilities", "phone"),
        4813 => ("bills_utilities", "phone"),
        4814 => ("bills_utilities", "phone"),    // Fax services
        4816 => ("bills_utilities", "internet"), // Computer network services
        4899 => ("bills_utilities", "utilities"), // Cable / pay TV
        4900 => ("bills_utilities", "utilities"), // Electric, gas, water

        // Insurance
        6300 => ("insurance", "insurance"),
        6381 => ("insurance", "insurance"),
        6399 => ("insurance", "insurance"),

        // Medical / Health
        5912 => ("health_wellness", "pharmacy"),
        8011 => ("health_wellness", "doctor"),
        8021 => ("health_wellness", "dentist"),
        8031 => ("health_wellness", "doctor"),
        8041 => ("health_wellness", "doctor"), // Chiropractors
        8042 => ("health_wellness", "vision"),
        8043 => ("health_wellness", "vision"),
        8049 => ("health_wellness", "doctor"), // Podiatrists
        8050 => ("health_wellness", "doctor"), // Nursing facilities
        8062 => ("health_wellness", "hospital"),
        8071 => ("health_wellness", "doctor"), // Medical/dental labs
        8099 => ("health_wellness", "doctor"),

        // Shopping - General
        5300 => ("shopping", "general_merchandise"),
        5310 => ("shopping", "general_merchandise"), // Discount stores
        5311 => ("shopping", "general_merchandise"), // Department stores
        5331 => ("shopping", "general_merchandise"), // Variety stores
        5399 => ("shopping", "general_merchandise"),

        // Shopping - Clothing
        5611 => ("shopping", "clothing"),
        5621 => ("shopping", "clothing"),
        5631 => ("shopping", "clothing"),
        5641 => ("shopping", "clothing"), // Children's wear
        5651 => ("shopping", "clothing"), // Family clothing
        5661 => ("shopping", "clothing"), // Shoe stores
        5691 => ("shopping", "clothing"),
        5699 => ("shopping", "clothing"),

        // Shopping - Electronics
        5722 => ("shopping", "electronics"), // Household appliance
        5732 => ("shopping", "electronics"),
        5734 => ("shopping", "electronics"), // Computer software
        5735 => ("shopping", "electronics"), // Music stores
        5045 => ("shopping", "electronics"), // Computers

        // Shopping - Home
        5200 => ("home", "home_improvement"),
        5211 => ("home", "home_improvement"), // Lumber, building materials
        5231 => ("home", "home_improvement"), // Glass, paint, wallpaper
        5251 => ("home", "home_improvement"), // Hardware stores
        5261 => ("home", "home_improvement"), // Nurseries / lawn & garden
        5712 => ("home", "furniture"),
        5713 => ("home", "furniture"), // Floor covering
        5714 => ("home", "furniture"), // Drapery / window covering
        5719 => ("home", "furniture"),

        // Entertainment
        7832 => ("entertainment", "movies"),
        7841 => ("entertainment", "movies"), // Video rental
        7911 => ("entertainment", "entertainment"),
        7922 => ("entertainment", "entertainment"), // Theatrical producers
        7929 => ("entertainment", "entertainment"), // Bands / orchestras
        7933 => ("entertainment", "entertainment"), // Bowling alleys
        7941 => ("entertainment", "entertainment"), // Sports clubs
        7991 => ("entertainment", "entertainment"), // Tourist attractions
        7993 => ("entertainment", "entertainment"), // Video game arcades
        7994 => ("entertainment", "entertainment"), // Video game arcades
        7996 => ("entertainment", "entertainment"), // Amusement parks
        7997 => ("recreation", "gym_fitness"),      // Membership clubs
        7998 => ("entertainment", "entertainment"), // Aquariums
        7999 => ("entertainment", "entertainment"),

        // Education
        8211 => ("education", "tuition"),
        8220 => ("education", "tuition"), // Colleges
        8241 => ("education", "tuition"), // Correspondence schools
        8244 => ("education", "tuition"), // Business schools
        8249 => ("education", "tuition"), // Trade schools
        8299 => ("education", "tuition"),

        // Personal Care
        7230 => ("personal_care", "hair_beauty"),
        7297 => ("personal_care", "hair_beauty"), // Massage parlors
        7298 => ("personal_care", "hair_beauty"), // Health and beauty spas

        // Pets
        742 => ("pets", "veterinary"),
        5995 => ("pets", "pet_supplies"),

        // Charitable / Religious
        8661 => ("gifts_donations", "charity"),
        8398 => ("gifts_donations", "charity"),

        // Government / Taxes
        9211 => ("taxes_fees", "government"),
        9222 => ("taxes_fees", "government"),
        9311 => ("taxes_fees", "government"),
        9399 => ("taxes_fees", "government"),
        9402 => ("taxes_fees", "government"), // Postal services
        9405 => ("taxes_fees", "government"),

        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcc_grocery_maps_correctly() {
        let result = mcc_to_category(5411);
        assert_eq!(result, Some(("food_dining", "groceries")));
    }

    #[test]
    fn mcc_restaurant_maps_correctly() {
        let result = mcc_to_category(5812);
        assert_eq!(result, Some(("food_dining", "restaurants")));
    }

    #[test]
    fn mcc_gas_maps_correctly() {
        let result = mcc_to_category(5541);
        assert_eq!(result, Some(("transportation", "gas_fuel")));
    }

    #[test]
    fn unknown_mcc_returns_none() {
        let result = mcc_to_category(9999);
        assert!(result.is_none());
    }

    #[test]
    fn parse_mcc_json_works() {
        let json = r#"[
            {"mcc": "5411", "edited_description": "Grocery Stores", "id": 1},
            {"mcc": "5812", "edited_description": "Eating Places", "id": 2}
        ]"#;
        let map = parse_mcc_json(json).unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&5411).unwrap(), "Grocery Stores");
    }
}
