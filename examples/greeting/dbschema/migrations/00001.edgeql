CREATE MIGRATION m13iunwvmjlnvhfk7cyywgtfjioihv4o434sf3m5h6sx6yfngho44a
    ONTO initial
{
  CREATE EXTENSION webassembly VERSION '0.1';
  CREATE TYPE default::Counter {
      CREATE PROPERTY value -> std::int64;
  };
  INSERT default::Counter { value := 1 };
};
