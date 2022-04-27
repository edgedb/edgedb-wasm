CREATE MIGRATION m17brpz5n3dg7ezsuh7gjhioe3ophbe7s2xkjjfky4762pmswx3klq
    ONTO initial
{
  CREATE TYPE default::Counter {
      CREATE REQUIRED PROPERTY name -> std::str {
          CREATE CONSTRAINT std::exclusive;
      };
      CREATE REQUIRED PROPERTY value -> std::int32 {
          SET default := 0;
      };
  };
};
