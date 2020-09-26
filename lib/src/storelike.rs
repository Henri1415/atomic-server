//! Trait for all stores to use

use crate::urls;
use crate::{
    collections::Collection, collections::Page, collections::TPFQuery, delta::Delta,
    errors::AtomicResult,
};
use crate::{
    datatype::{match_datatype, DataType},
    mapping::Mapping,
    resources::{self, ResourceString},
    values::Value,
    Atom, Resource, RichAtom,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Property {
    // URL of the class
    pub class_type: Option<String>,
    // URL of the datatype
    pub data_type: DataType,
    pub shortname: String,
    pub subject: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Class {
    pub requires: Vec<Property>,
    pub recommends: Vec<Property>,
    pub shortname: String,
    pub description: String,
    /// URL
    pub subject: String,
}

// A path can return one of many things
pub enum PathReturn {
    Subject(String),
    Atom(Box<RichAtom>),
}

pub type ResourceCollection = Vec<(String, ResourceString)>;

/// Storelike provides many useful methods for interacting with an Atomic Store.
/// It serves as a basic store Trait, agnostic of how it functions under the hood.
/// This is useful, because we can create methods for Storelike that will work with either in-memory
/// stores, as well as with persistend on-disk stores.
pub trait Storelike {
    /// Add individual Atoms to the store.
    /// Will replace existing Atoms that share Subject / Property combination.
    fn add_atoms(&self, atoms: Vec<Atom>) -> AtomicResult<()>;

    /// Replaces existing resource with the contents
    /// Accepts a simple nested string only hashmap
    /// Adds to hashmap and to the resource store
    fn add_resource_string(
        &self,
        subject: String,
        resource: &ResourceString,
    ) -> AtomicResult<()>;

    /// Returns a hashmap ResourceString with string Values.
    /// Fetches the resource if it is not in the store.
    fn get_resource_string(&self, subject: &str) -> AtomicResult<ResourceString>;

    /// Returns the root URL where the store is hosted.
    /// E.g. `https://example.com`
    /// This is where deltas should be sent to.
    /// Also useful for Subject URL generation.
    fn get_base_url(&self) -> Option<String> {
        None
    }

    /// Adds a Resource to the store
    fn add_resource(&self, resource: &Resource) -> AtomicResult<()> {
        self.add_resource_string(resource.get_subject().clone(), &resource.to_plain())?;
        Ok(())
    }

    /// Fetches a resource, makes sure its subject matches.
    /// Save to the store.
    /// Only adds atoms with matching subjects will be added.
    fn fetch_resource(&self, subject: &str) -> AtomicResult<ResourceString> {
        let resource: ResourceString = crate::client::fetch_resource(subject)?;
        self.add_resource_string(subject.into(), &resource)?;
        Ok(resource)
    }

    /// Returns a full Resource with native Values
    fn get_resource(&self, subject: &str) -> AtomicResult<Resource> where Self: std::marker::Sized {
        let resource_string = self.get_resource_string(subject)?;
        let mut res = Resource::new(subject.into(), self);
        for (prop_string, val_string) in resource_string {
            let propertyfull = self.get_property(&prop_string)?;
            let fullvalue =
                Value::new(&val_string, &propertyfull.data_type).expect("Could not convert value");
            res.set_propval(prop_string.clone(), fullvalue)?;
        }
        Ok(res)
        // Above code is a copy from:
        // let res = Resource::new_from_resource_string(subject.clone(), &resource, self)?;
        // But has some Size issues
    }

    /// Retrieves a Class from the store by subject URL and converts it into a Class useful for forms
    fn get_class(&self, subject: &str) -> AtomicResult<Class> {
        // The string representation of the Class
        let class_strings = self.get_resource_string(subject).expect("Class not found");
        let shortname = class_strings
            .get(urls::SHORTNAME)
            .expect("Class has no shortname");
        let description = class_strings
            .get(urls::DESCRIPTION)
            .expect("Class has no description");
        let requires_string = class_strings.get(urls::REQUIRES);
        let recommends_string = class_strings.get(urls::RECOMMENDS);

        let mut requires: Vec<Property> = Vec::new();
        let mut recommends: Vec<Property> = Vec::new();
        let get_properties = |resource_array: &str| -> Vec<Property> {
            let mut properties: Vec<Property> = vec![];
            let string_vec: Vec<String> = crate::parse::parse_json_array(&resource_array).unwrap();
            for prop_url in string_vec {
                properties.push(self.get_property(&prop_url).unwrap());
            }
            properties
        };
        if let Some(string) = requires_string {
            requires = get_properties(string);
        }
        if let Some(string) = recommends_string {
            recommends = get_properties(string);
        }
        let class = Class {
            requires,
            recommends,
            shortname: shortname.into(),
            subject: subject.into(),
            description: description.into(),
        };

        Ok(class)
    }

    /// Finds all classes (isA) for any subject.
    /// Returns an empty vector if there are none.
    fn get_classes_for_subject(&self, subject: &str) -> AtomicResult<Vec<Class>> {
        let resource = self.get_resource_string(subject)?;
        let classes_array_opt = resource.get(urls::IS_A);
        let classes_array = match classes_array_opt {
            Some(vec) => vec,
            None => return Ok(Vec::new()),
        };
        // .ok_or(format!("IsA property not present in {}", subject))?;
        let native = Value::new(classes_array, &DataType::ResourceArray)?;
        let vector = match native {
            Value::ResourceArray(vec) => vec,
            _ => return Err("Should be an array".into()),
        };
        let mut classes: Vec<Class> = Vec::new();
        for class in vector {
            classes.push(self.get_class(&class)?)
        }
        Ok(classes)
    }

    /// Constructs a Collection, which is a paginated list of items with some sorting applied.
    fn get_collection(
        &self,
        tpf: TPFQuery,
        sort_by: String,
        sort_desc: bool,
        _page_nr: u8,
        _page_size: u8,
    ) -> AtomicResult<Collection> {
        // Execute the TPF query, get all the subjects.
        let atoms = self.tpf(None, tpf.property.as_deref(), tpf.value.as_deref())?;
        // Iterate over the fetched resources
        let subjects: Vec<String> = atoms.iter().map(|atom| atom.subject.clone()).collect();
        let mut resources: Vec<ResourceString> = Vec::new();
        for sub in subjects.clone() {
            resources.push(self.get_resource_string(&sub)?);
        }
        // Sort the resources (TODO), use sortBy and sortDesc
        let sorted_subjects: Vec<String> = subjects.clone();
        // Construct the pages (TODO), use pageSize
        let mut pages: Vec<Page> = Vec::new();
        // Construct the requested page (TODO)
        let page = Page {
            members: sorted_subjects,
        };
        pages.push(page);
        let collection = Collection {
            tpf,
            total_pages: pages.len() as u8,
            pages,
            sort_by,
            sort_desc,
            current_page: 0,
            total_items: subjects.len() as u8,
            page_size: subjects.len() as u8,
        };
        Ok(collection)
    }

    /// Fetches a property by URL, returns a Property instance
    fn get_property(&self, url: &str) -> AtomicResult<Property> {
        let property_resource = self.get_resource_string(url)?;
        let property = Property {
            data_type: match_datatype(
                &property_resource
                    .get(urls::DATATYPE_PROP)
                    .ok_or(format!("Datatype not found for property {}", url))?,
            ),
            shortname: property_resource
                .get(urls::SHORTNAME)
                .ok_or(format!("Shortname not found for property {}", url))?
                .into(),
            description: property_resource
                .get(urls::DESCRIPTION)
                .ok_or(format!("Description not found for property {}", url))?
                .into(),
            class_type: property_resource.get(urls::CLASSTYPE_PROP).cloned(),
            subject: url.into(),
        };

        Ok(property)
    }

    /// Returns a collection with all resources in the store.
    /// WARNING: This could be very expensive!
    fn all_resources(&self) -> ResourceCollection;

    /// Adds an atom to the store. Does not do any validations
    fn add_atom(&self, atom: Atom) -> AtomicResult<()> {
        match self.get_resource_string(&atom.subject).as_mut() {
            Ok(resource) => {
                // Overwrites existing properties
                if let Some(_oldval) = resource.insert(atom.property, atom.value) {
                    // Remove the value from the Subject index
                    // self.index_value_remove(atom);
                };
                self.add_resource_string(atom.subject, &resource)?;
            }
            Err(_) => {
                let mut resource: ResourceString = HashMap::new();
                resource.insert(atom.property, atom.value);
                self.add_resource_string(atom.subject, &resource)?;
            }
        };
        Ok(())
    }
    /// Processes a vector of deltas and updates the store.
    /// Panics if the
    /// Use this for ALL updates to the store!
    fn process_delta(&self, delta: Delta) -> AtomicResult<()> {
        let mut updated_resources = Vec::new();

        for deltaline in delta.lines.into_iter() {
            match deltaline.method.as_str() {
                urls::INSERT | "insert" => {
                    let datatype = self
                        .get_property(&deltaline.property)
                        .expect("Can't get property")
                        .data_type;
                    Value::new(&deltaline.value, &datatype)?;
                    updated_resources.push(delta.subject.clone());
                    let atom =
                        Atom::new(delta.subject.clone(), deltaline.property, deltaline.value);
                    self.add_atom(atom)?;
                }
                urls::DELETE | "delete" => {
                    todo!();
                }
                unknown => println!("Ignoring unknown method: {}", unknown),
            };
        }
        Ok(())
    }
    /// Finds the URL of a shortname used in the context of a specific Resource.
    /// The Class, Properties and Shortnames of the Resource are used to find this URL
    fn property_shortname_to_url(
        &self,
        shortname: &str,
        resource: &ResourceString,
    ) -> AtomicResult<String> {
        for (prop_url, _value) in resource.iter() {
            let prop_resource = self.get_resource_string(&*prop_url)?;
            let prop_shortname = prop_resource
                .get(urls::SHORTNAME)
                .ok_or(format!("Property shortname for '{}' not found", prop_url))?;
            if prop_shortname == shortname {
                return Ok(prop_url.clone());
            }
        }
        Err(format!("Could not find shortname {}", shortname).into())
    }

    /// Finds
    fn property_url_to_shortname(&self, url: &str) -> AtomicResult<String> {
        let resource = self.get_resource_string(url)?;
        let property_resource = resource
            .get(urls::SHORTNAME)
            .ok_or(format!("Could not get shortname prop for {}", url))?;

        Ok(property_resource.into())
    }

    /// fetches a resource, serializes it to .ad3
    fn resource_to_ad3(&self, subject: &str) -> AtomicResult<String> {
        let mut string = String::new();
        let resource = self.get_resource_string(subject)?;

        for (property, value) in resource {
            let mut ad3_atom = serde_json::to_string(&vec![subject, &property, &value])?;
            ad3_atom.push_str("\n");
            string.push_str(&*ad3_atom);
        }
        Ok(string)
    }

    /// Serializes a single Resource to a JSON object.
    /// It uses the Shortnames of properties for Keys.
    /// The depth is useful, since atomic data allows for cyclical (infinite-depth) relationships
    // Todo:
    // [ ] Resources into objects, if the nesting depth allows it
    fn resource_to_json(
        &self,
        resource_url: &str,
        // Not yet used
        _depth: u8,
        json_ld: bool,
    ) -> AtomicResult<String> {
        use serde_json::{Map, Value as SerdeValue};

        let resource = self.get_resource_string(resource_url)?;

        // Initiate JSON object
        let mut root = Map::new();

        // For JSON-LD serialization
        let mut context = Map::new();

        // For every atom, find the key, datatype and add it to the @context
        for (prop_url, value) in resource.iter() {
            // We need the Property for shortname and Datatype
            let property = self.get_property(prop_url)?;
            if json_ld {
                // In JSON-LD, the value of a Context Item can be a string or an object.
                // This object can contain information about the translation or datatype of the value
                let ctx_value: SerdeValue = match property.data_type {
                    DataType::AtomicUrl => {
                        let mut obj = Map::new();
                        obj.insert("@id".into(), prop_url.as_str().into());
                        obj.insert("@type".into(), "@id".into());
                        obj.into()
                    }
                    DataType::Date => {
                        let mut obj = Map::new();
                        obj.insert("@id".into(), prop_url.as_str().into());
                        obj.insert(
                            "@type".into(),
                            "http://www.w3.org/2001/XMLSchema#date".into(),
                        );
                        obj.into()
                    }
                    DataType::Integer => {
                        let mut obj = Map::new();
                        obj.insert("@id".into(), prop_url.as_str().into());
                        // I'm not sure whether we should use XSD or Atomic Datatypes
                        obj.insert(
                            "@type".into(),
                            "http://www.w3.org/2001/XMLSchema#integer".into(),
                        );
                        obj.into()
                    }
                    DataType::Markdown => prop_url.as_str().into(),
                    DataType::ResourceArray => {
                        let mut obj = Map::new();
                        obj.insert("@id".into(), prop_url.as_str().into());
                        // Plain JSON-LD Arrays are not ordered. Here, they are converted into an RDF List.
                        obj.insert("@container".into(), "@list".into());
                        obj.into()
                    }
                    DataType::Slug => prop_url.as_str().into(),
                    DataType::String => prop_url.as_str().into(),
                    DataType::Timestamp => prop_url.as_str().into(),
                    DataType::Unsupported(_) => prop_url.as_str().into(),
                };
                context.insert(property.shortname.as_str().into(), ctx_value);
            }
            let native_value = Value::new(value, &property.data_type).expect(&*format!(
                "Could not convert value {:?} with property type {:?} into native value",
                value, &property.data_type
            ));
            let jsonval = match native_value {
                Value::AtomicUrl(val) => SerdeValue::String(val),
                Value::Date(val) => SerdeValue::String(val),
                Value::Integer(val) => SerdeValue::Number(val.into()),
                Value::Markdown(val) => SerdeValue::String(val),
                Value::ResourceArray(val) => SerdeValue::Array(
                    val.iter()
                        .map(|item| SerdeValue::String(item.clone()))
                        .collect(),
                ),
                Value::Slug(val) => SerdeValue::String(val),
                Value::String(val) => SerdeValue::String(val),
                Value::Timestamp(val) => SerdeValue::Number(val.into()),
                Value::Unsupported(val) => SerdeValue::String(val.value),
            };
            root.insert(property.shortname, jsonval);
        }

        if json_ld {
            root.insert("@context".into(), context.into());
            root.insert("@id".into(), resource_url.into());
        }
        let obj = SerdeValue::Object(root);
        let string = serde_json::to_string_pretty(&obj).expect("Could not serialize to JSON");

        Ok(string)
    }

    /// Triple Pattern Fragments interface.
    /// Use this for most queries, e.g. finding all items with some property / value combination.
    /// Returns an empty array if nothing is found.
    ///
    /// # Example
    ///
    /// For example, if I want to view all Resources that are instances of the class "Property", I'd do:
    ///
    /// ```
    /// use atomic_lib::Storelike;
    /// let mut store = atomic_lib::Store::init();
    /// store.populate();
    /// let atoms = store.tpf(
    ///     None,
    ///     Some("https://atomicdata.dev/properties/isA"),
    ///     Some("[\"https://atomicdata.dev/classes/Class\"]")
    /// ).unwrap();
    /// assert!(atoms.len() == 3)
    /// ```
    // Very costly, slow implementation.
    // Does not assume any indexing.
    fn tpf(
        &self,
        q_subject: Option<&str>,
        q_property: Option<&str>,
        q_value: Option<&str>,
    ) -> AtomicResult<Vec<Atom>> {
        let mut vec: Vec<Atom> = Vec::new();

        let hassub = q_subject.is_some();
        let hasprop = q_property.is_some();
        let hasval = q_value.is_some();

        // Simply return all the atoms
        if !hassub && !hasprop && !hasval {
            for (sub, resource) in self.all_resources() {
                for (property, value) in resource {
                    vec.push(Atom::new(sub.clone(), property, value))
                }
            }
            return Ok(vec);
        }

        // If the value is a resourcearray, check if it is inside
        let val_equals = |val: &str| {
            let q = q_value.unwrap();
            val == q || {
                if val.starts_with('[') {
                match crate::parse::parse_json_array(val) {
                        Ok(vec) => {
                            return vec.contains(&q.into())
                        }
                        Err(_) => return val == q
                    }
                }
                false
            }
        };

        // Find atoms matching the TPF query in a single resource
        let mut find_in_resource = |subj: &str, resource: &ResourceString| {
            for (prop, val) in resource.iter() {
                if hasprop && q_property.as_ref().unwrap() == prop {
                    if hasval {
                        if val_equals(val) {
                            vec.push(Atom::new(subj.into(), prop.into(), val.into()))
                        }
                    } else {
                        vec.push(Atom::new(subj.into(), prop.into(), val.into()))
                    }
                } else if hasval && val_equals(val) {
                    vec.push(Atom::new(subj.into(), prop.into(), val.into()))
                }
            }
        };

        match q_subject {
            Some(sub) => match self.get_resource_string(&sub) {
                Ok(resource) => {
                    if q_property.is_some() | q_value.is_some() {
                        find_in_resource(&sub, &resource);
                        Ok(vec)
                    } else {
                        Ok(resources::resourcestring_to_atoms(sub, resource))
                    }
                }
                Err(_) => Ok(vec),
            },
            None => {
                for (subj, properties) in self.all_resources() {
                    find_in_resource(&subj, &properties);
                }
                Ok(vec)
            }
        }
    }

    /// Accepts an Atomic Path string, returns the result value (resource or property value)
    /// E.g. `https://example.com description` or `thing isa 0`
    /// https://docs.atomicdata.dev/core/paths.html
    //  Todo: return something more useful, give more context.
    fn get_path(
        &self,
        atomic_path: &str,
        mapping: Option<&Mapping>,
    ) -> AtomicResult<PathReturn> {
        // The first item of the path represents the starting Resource, the following ones are traversing the graph / selecting properties.
        let path_items: Vec<&str> = atomic_path.split(' ').collect();
        let first_item = String::from(path_items[0]);
        let mut id_url = first_item;
        if mapping.is_some() {
            // For the first item, check the user mapping
            id_url = mapping
                .unwrap()
                .try_mapping_or_url(&id_url)
                .ok_or(&*format!("No url found for {}", path_items[0]))?;
        }
        if path_items.len() == 1 {
            return Ok(PathReturn::Subject(id_url));
        }
        // The URL of the next resource
        let mut subject = id_url;
        // Set the currently selectred resource parent, which starts as the root of the search
        let mut resource = self.get_resource_string(&subject)?;
        // During each of the iterations of the loop, the scope changes.
        // Try using pathreturn...
        let mut current: PathReturn = PathReturn::Subject(subject.clone());
        // Loops over every item in the list, traverses the graph
        // Skip the first one, for that is the subject (i.e. first parent) and not a property
        for item in path_items[1..].iter().cloned() {
            // In every iteration, the subject, property_url and current should be set.
            // Ignore double spaces
            if item == "" {
                continue;
            }
            // If the item is a number, assume its indexing some array
            if let Ok(i) = item.parse::<u32>() {
                match current {
                    PathReturn::Atom(atom) => {
                        // let resource_check = resource.ok_or("Resource not found")?;
                        let array_string = resource
                            .get(&atom.property.subject)
                            .ok_or(format!("Property {} not found", &atom.property.subject))?;
                        let vector: Vec<String> = crate::parse::parse_json_array(array_string)
                            .expect(&*format!("Failed to parse array: {}", array_string));
                        if vector.len() <= i as usize {
                            eprintln!(
                                "Too high index ({}) for array with length {}",
                                i,
                                array_string.len()
                            );
                        }
                        let url = &vector[i as usize];
                        subject = url.into();
                        resource = self.get_resource_string(&subject)?;
                        current = PathReturn::Subject(subject.clone());
                        continue;
                    }
                    PathReturn::Subject(_) => {
                        return Err("You can't do an index on a resource, only on arrays.".into())
                    }
                }
            }
            // Since the selector isn't an array index, we can assume it's a property URL
            let property_url;
            match current {
                PathReturn::Subject(_) => {}
                PathReturn::Atom(_) => {
                    return Err("No more linked resources down this path.".into())
                }
            }
            // Get the shortname or use the URL
            if crate::mapping::is_url(item) {
                property_url = item.into();
            } else {
                // Traverse relations, don't use mapping here, but do use classes
                property_url = self.property_shortname_to_url(item, &resource)?;
            }
            // Set the parent for the next loop equal to the next node.
            // TODO: skip this step if the current iteration is the last one
            let value = resource.get(&property_url).unwrap();
            let property = self.get_property(&property_url)?;
            current = PathReturn::Atom(Box::new(RichAtom::new(
                subject.clone(),
                property,
                value.clone(),
            )?))
        }
        Ok(current)
    }

    /// Loads the default store
    fn populate(&self) -> AtomicResult<()> {
        let ad3 = include_str!("../defaults/default_store.ad3");
        let atoms = crate::parse::parse_ad3(&String::from(ad3))?;
        self.add_atoms(atoms)?;
        Ok(())
    }

    /// Performs a light validation, without fetching external data
    fn validate(&self) -> crate::validate::ValidationReport where Self: std::marker::Sized {
        crate::validate::validate_store(self, false)
    }
}
