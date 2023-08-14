package com.tomcat.domain;

import lombok.Data;

import javax.persistence.*;

@Entity
@Data
@Table(name = "user_profile")
public class UserProfile {

  @Id
  private String id;

  @OneToOne
  @JoinColumn(name="userId", unique = true)
  private User user;

  @Column(columnDefinition = "varchar(255) CHARACTER SET utf8mb4")
  private String motherTongue;

  @Column(columnDefinition = "varchar(255) CHARACTER SET utf8mb4")
  private String languageDepth;

  @Column(columnDefinition = "varchar(255) CHARACTER SET utf8mb4")
  private String communicationStyle;

  @Column(columnDefinition = "varchar(255) CHARACTER SET utf8mb4")
  private String targetLanguage;

  // constructor, getter and setter


  public void setUser(User user) {
    this.user = user;
    this.id = user.getId();
  }
}